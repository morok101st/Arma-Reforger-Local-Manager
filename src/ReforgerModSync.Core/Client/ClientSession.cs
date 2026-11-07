using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace ReforgerModSync.Core.Client;

/// <summary>Ein laufender Client. Ein Job Object beendet ihn mit, falls das Tool abstürzt.</summary>
public sealed class ClientSession : IClientProcess
{
    private static readonly TimeSpan HideIntervalAtStart = TimeSpan.FromMilliseconds(50);
    private static readonly TimeSpan HideInterval = TimeSpan.FromMilliseconds(500);
    private static readonly TimeSpan StartPhase = TimeSpan.FromSeconds(30);

    private readonly Process _process;
    private readonly SafeFileHandle _job;
    private readonly CancellationTokenSource _hideCancel = new();

    private ClientSession(Process process, SafeFileHandle job)
    {
        _process = process;
        _job = job;
    }

    public bool HasExited => _process.HasExited;

    /// <param name="hideWindow">Blendet das Spielfenster aus. Es bleibt beim Start nur einen Moment sichtbar.</param>
    public static ClientSession Start(string exe, IEnumerable<string> arguments, bool hideWindow)
    {
        var startInfo = new ProcessStartInfo(exe)
        {
            UseShellExecute = false,
            WorkingDirectory = Path.GetDirectoryName(exe)!,
        };
        foreach (var argument in arguments)
        {
            startInfo.ArgumentList.Add(argument);
        }

        var job = CreateKillOnCloseJob();
        var process = Process.Start(startInfo) ?? throw new InvalidOperationException("Der Client konnte nicht gestartet werden.");
        AssignProcessToJobObject(job, process.Handle);

        var session = new ClientSession(process, job);
        if (hideWindow)
        {
            _ = session.KeepHiddenAsync();
        }

        return session;
    }

    /// <summary>Erst sauber schließen, sonst den ganzen Prozessbaum beenden.</summary>
    public async Task StopAsync()
    {
        _hideCancel.Cancel();
        if (_process.HasExited)
        {
            return;
        }

        // Nicht CloseMainWindow: Für ein ausgeblendetes Fenster kennt .NET kein Hauptfenster mehr.
        foreach (var window in WindowsOf(_process.Id))
        {
            PostMessage(window, WmClose, IntPtr.Zero, IntPtr.Zero);
        }

        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        try
        {
            await _process.WaitForExitAsync(timeout.Token);
        }
        catch (OperationCanceledException)
        {
            _process.Kill(entireProcessTree: true);
            await _process.WaitForExitAsync();
        }
    }

    public void Dispose()
    {
        _hideCancel.Cancel();
        _hideCancel.Dispose();
        _job.Dispose();
        _process.Dispose();
    }

    /// <summary>Läuft dauerhaft, weil das Spiel beim Laden weitere Fenster öffnen oder seines wieder zeigen kann.</summary>
    private async Task KeepHiddenAsync()
    {
        var processId = _process.Id;
        var token = _hideCancel.Token;
        var watch = Stopwatch.StartNew();
        try
        {
            while (!token.IsCancellationRequested)
            {
                foreach (var window in WindowsOf(processId))
                {
                    if (IsWindowVisible(window))
                    {
                        ShowWindow(window, SwHide);
                    }
                }

                await Task.Delay(watch.Elapsed < StartPhase ? HideIntervalAtStart : HideInterval, token);
            }
        }
        catch (OperationCanceledException)
        {
            // Normales Ende der Sitzung.
        }
    }

    private static List<IntPtr> WindowsOf(int processId)
    {
        var windows = new List<IntPtr>();
        EnumWindows(
            (window, _) =>
            {
                GetWindowThreadProcessId(window, out var owner);
                if (owner == processId)
                {
                    windows.Add(window);
                }

                return true;
            },
            IntPtr.Zero);
        return windows;
    }

    private static SafeFileHandle CreateKillOnCloseJob()
    {
        var job = CreateJobObject(IntPtr.Zero, null);
        if (job.IsInvalid)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }

        var info = new JobObjectExtendedLimitInformation { LimitFlags = JobObjectLimitKillOnJobClose };
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformationClass, ref info, Marshal.SizeOf<JobObjectExtendedLimitInformation>()))
        {
            var error = Marshal.GetLastWin32Error();
            job.Dispose();
            throw new Win32Exception(error);
        }

        return job;
    }

    private const int SwHide = 0;
    private const uint WmClose = 0x0010;
    private const uint JobObjectLimitKillOnJobClose = 0x2000;
    private const int JobObjectExtendedLimitInformationClass = 9;

    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr window, int command);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateJobObject(IntPtr attributes, string? name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(
        SafeFileHandle job,
        int infoClass,
        ref JobObjectExtendedLimitInformation info,
        int length);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(SafeFileHandle job, IntPtr process);

    [StructLayout(LayoutKind.Sequential)]
    private struct JobObjectExtendedLimitInformation
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }
}
