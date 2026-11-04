using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Imaging;
using H.NotifyIcon;
using H.NotifyIcon.Core;

namespace ReforgerModSync.App;

/// <summary>Symbol im Infobereich: öffnet die App, startet den Sync und beendet die App.</summary>
public sealed class TrayIcon : IDisposable
{
    private readonly MainViewModel _viewModel;
    private readonly MainWindow _window;
    private readonly TaskbarIcon _icon;
    private readonly MenuItem _syncItem;
    private bool _hintShown;

    public TrayIcon(MainViewModel viewModel, MainWindow window)
    {
        _viewModel = viewModel;
        _window = window;

        var openItem = new MenuItem { Header = "Öffnen", FontWeight = FontWeights.SemiBold };
        openItem.Click += (_, _) => ShowWindow();

        _syncItem = new MenuItem { Header = viewModel.SyncButtonText };
        _syncItem.Click += (_, _) => viewModel.SyncCommand.Execute(null);

        var exitItem = new MenuItem { Header = "Beenden" };
        exitItem.Click += (_, _) => Exit();

        var menu = new ContextMenu();
        menu.Items.Add(openItem);
        menu.Items.Add(_syncItem);
        menu.Items.Add(new Separator());
        menu.Items.Add(exitItem);

        _icon = new TaskbarIcon
        {
            IconSource = new BitmapImage(new Uri("pack://application:,,,/Assets/app.ico")),
            ToolTipText = viewModel.TrayStatus,
            ContextMenu = menu,
            MenuActivation = PopupActivationMode.RightClick,
        };
        _icon.TrayLeftMouseDown += (_, _) => ShowWindow();
        _icon.ForceCreate(enablesEfficiencyMode: false);

        viewModel.PropertyChanged += OnViewModelChanged;
        viewModel.RunFinished += ShowNotification;
        window.HiddenToTray += OnHiddenToTray;
    }

    public void ShowWindow()
    {
        _window.Show();
        if (_window.WindowState == WindowState.Minimized)
        {
            _window.WindowState = WindowState.Normal;
        }

        _window.Activate();
    }

    public void Dispose()
    {
        _viewModel.PropertyChanged -= OnViewModelChanged;
        _viewModel.RunFinished -= ShowNotification;
        _icon.Dispose();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(MainViewModel.TrayStatus):
                _icon.ToolTipText = _viewModel.TrayStatus;
                break;
            case nameof(MainViewModel.SyncButtonText):
                _syncItem.Header = _viewModel.SyncButtonText;
                break;
        }
    }

    /// <summary>Nur wenn das Fenster versteckt ist. Sonst sieht der Nutzer das Ergebnis schon in der App.</summary>
    private void ShowNotification(string title, string message, bool isError)
    {
        if (!_window.IsVisible)
        {
            _icon.ShowNotification(title, message, isError ? NotificationIcon.Error : NotificationIcon.Info);
        }
    }

    private void OnHiddenToTray()
    {
        if (_hintShown)
        {
            return;
        }

        _hintShown = true;
        _icon.ShowNotification("Sync & Launch","Die App läuft im Infobereich weiter. Beenden über das Symbol mit der rechten Maustaste.", NotificationIcon.Info);
    }

    private void Exit()
    {
        _window.AllowClose();
        Application.Current.Shutdown();
    }
}
