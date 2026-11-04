using System.IO;
using System.Windows;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Credentials;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Sync;
using Serilog;

namespace ReforgerModSync.App;

public partial class App : Application
{
    private const string InstanceMutexName = @"Local\ReforgerModSync.App";
    private const string ShowWindowEventName = @"Local\ReforgerModSync.App.ShowWindow";

    private Mutex? _instanceMutex;
    private EventWaitHandle? _showWindowEvent;
    private RegisteredWaitHandle? _showWindowWait;
    private MainViewModel? _viewModel;
    private bool _startupFinished;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Es läuft nur eine Instanz: Ein zweiter Start holt das vorhandene Fenster nach vorn.
        _instanceMutex = new Mutex(true, InstanceMutexName, out var isFirstInstance);
        if (!isFirstInstance)
        {
            if (EventWaitHandle.TryOpenExisting(ShowWindowEventName, out var running))
            {
                running.Set();
                running.Dispose();
            }

            Shutdown();
            return;
        }

        Log.Logger = new LoggerConfiguration()
            .WriteTo.File(
                Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "ReforgerModSync",
                    "logs",
                    "app-.log"),
                rollingInterval: RollingInterval.Day,
                retainedFileCountLimit: 14,
                flushToDiskInterval: TimeSpan.FromSeconds(1))
            .CreateLogger();
        Log.Information("Reforger Sync & Launch {Version} gestartet", MainViewModel.Version);

        DispatcherUnhandledException += (_, args) =>
        {
            Log.Error(args.Exception, "Unerwarteter Fehler");
            BrandDialog.Inform(MainWindow, "Unerwarteter Fehler", args.Exception.Message);
            args.Handled = true;

            // Sonst liefe die App unsichtbar weiter, und die Sperre verhindert jeden weiteren Start.
            if (!_startupFinished)
            {
                Shutdown(1);
            }
        };

        GameLauncher.DeletePasswordFile();

        var credentials = new WindowsCredentialStore();
        var cache = new ModListCache(ModListCache.DefaultFolder);
        _viewModel = new MainViewModel(
            new SettingsStore(SettingsStore.DefaultFilePath),
            credentials,
            cache,
            new SyncOrchestrator(credentials, cache));

        var window = new MainWindow(_viewModel);
        window.Show();

        _showWindowEvent = new EventWaitHandle(false, EventResetMode.AutoReset, ShowWindowEventName);
        _showWindowWait = ThreadPool.RegisterWaitForSingleObject(
            _showWindowEvent,
            (_, _) => Dispatcher.BeginInvoke(window.BringToFront),
            null,
            Timeout.Infinite,
            executeOnlyOnce: false);
        _startupFinished = true;
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _showWindowWait?.Unregister(null);
        _showWindowEvent?.Dispose();

        var passwordWait = _viewModel?.PasswordCleanupWait ?? TimeSpan.Zero;
        if (passwordWait > TimeSpan.Zero)
        {
            // Direkt nach dem Beitritt geschlossen: Das Spiel braucht die Datei noch, liegen bleiben darf sie aber
            // nicht. Die Sperre muss ganz geschlossen sein, ein offenes Handle hielte einen neuen Start davon ab.
            _instanceMutex?.ReleaseMutex();
            _instanceMutex?.Dispose();
            _instanceMutex = null;
            GameLauncher.WaitUntilRemoved(GameLauncher.PasswordFile, passwordWait);
            GameLauncher.DeletePasswordFile();
        }

        _instanceMutex?.Dispose();
        Log.CloseAndFlush();
        base.OnExit(e);
    }
}
