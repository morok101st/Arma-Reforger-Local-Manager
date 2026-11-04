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
    private TrayIcon? _tray;
    private bool _startupFinished;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Die App läuft versteckt im Infobereich weiter: Ein zweiter Start holt nur das Fenster nach vorn.
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
        var viewModel = new MainViewModel(
            new SettingsStore(SettingsStore.DefaultFilePath),
            credentials,
            cache,
            new SyncOrchestrator(credentials, cache));

        // Das Schließen des Fensters beendet die App nicht, das geht über das Symbol im Infobereich.
        ShutdownMode = ShutdownMode.OnExplicitShutdown;
        var window = new MainWindow(viewModel);
        window.Show();

        // Das Symbol ist verzichtbar: Lehnt Windows es ab, läuft die App ohne und endet mit dem Fenster.
        try
        {
            _tray = new TrayIcon(viewModel, window);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Kein Symbol im Infobereich, die App endet mit dem Fenster");
            window.ExitOnClose();
            ShutdownMode = ShutdownMode.OnLastWindowClose;
        }

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
        _tray?.Dispose();
        _instanceMutex?.Dispose();
        Log.CloseAndFlush();
        base.OnExit(e);
    }
}
