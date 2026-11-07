using System.ComponentModel;
using System.Diagnostics;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Client;

/// <summary>
/// Startet das Spiel zum Spielen, mit dem echten Profil des Spielers und über den BattlEye-Starter,
/// wie es die Installation des Spiels selbst vorsieht. Es läuft bewusst unabhängig von der App weiter.
/// </summary>
public static class GameLauncher
{
    private const string BattlEyeStarter = "ArmaReforger_BE.exe";

    /// <summary>Startet das Spiel und lässt die Hilfs-Mod dem Server beitreten.</summary>
    /// <returns><c>null</c> bei Erfolg, sonst eine Meldung für den Nutzer.</returns>
    public static string? StartAndJoin(AppSettings settings, ServerProfile profile, string? password)
    {
        // Erst alles prüfen, dann die Passwortdatei schreiben: Sie soll nur existieren, wenn das Spiel auch startet.
        if (ClientPrecheck.Check(settings, profile, out var exe) is { } problem)
        {
            return problem;
        }

        PasswordFile.Prepare(PasswordFile.ForPlayerProfile, password);
        problem = Launch(exe, ClientArgsBuilder.BuildJoin(profile.ServerAddress!, profile.DownloadDir, AddonPaths.HelperRoot));
        if (problem is not null)
        {
            PasswordFile.TryDelete(PasswordFile.ForPlayerProfile);
        }

        return problem;
    }

    /// <summary>Wartet, bis die Datei weg ist, längstens <paramref name="maxWait"/>. Blockiert, nur für das App-Ende gedacht.</summary>
    /// <returns><c>true</c>, wenn die Datei nicht mehr existiert.</returns>
    public static bool WaitUntilRemoved(string path, TimeSpan maxWait)
    {
        var stopwatch = Stopwatch.StartNew();
        while (File.Exists(path))
        {
            if (stopwatch.Elapsed >= maxWait)
            {
                return false;
            }

            Thread.Sleep(TimeSpan.FromMilliseconds(100));
        }

        return true;
    }

    private static string? Launch(string exe, IReadOnlyList<string> arguments)
    {
        var folder = Path.GetDirectoryName(exe)!;
        var starter = Path.Combine(folder, BattlEyeStarter);
        var useStarter = File.Exists(starter);
        IEnumerable<string> all = useStarter ? ["-exe", Path.GetFileName(exe), .. arguments] : arguments;

        // ShellExecute, weil der BattlEye-Starter Administratorrechte anfordern kann.
        var startInfo = new ProcessStartInfo(useStarter ? starter : exe)
        {
            UseShellExecute = true,
            WorkingDirectory = folder,
            Arguments = ClientArgsBuilder.ToCommandLine(all),
        };

        try
        {
            using var process = Process.Start(startInfo);
            return null;
        }
        catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
        {
            return $"Das Spiel konnte nicht gestartet werden: {ex.Message}";
        }
    }
}
