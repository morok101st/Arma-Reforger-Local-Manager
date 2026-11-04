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
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        Log.Logger = new LoggerConfiguration()
            .WriteTo.File(
                Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "ReforgerModSync",
                    "logs",
                    "app-.log"),
                rollingInterval: RollingInterval.Day,
                retainedFileCountLimit: 14)
            .CreateLogger();

        DispatcherUnhandledException += (_, args) =>
        {
            Log.Error(args.Exception, "Unerwarteter Fehler");
            MessageBox.Show(args.Exception.Message, "101st Server Manager", MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true;
        };

        GameLauncher.DeletePasswordFile();

        var credentials = new WindowsCredentialStore();
        var cache = new ModListCache(ModListCache.DefaultFolder);
        var viewModel = new MainViewModel(
            new SettingsStore(SettingsStore.DefaultFilePath),
            credentials,
            cache,
            new SyncOrchestrator(credentials, cache));
        new MainWindow(viewModel).Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Log.CloseAndFlush();
        base.OnExit(e);
    }
}
