using System.ComponentModel;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using Microsoft.Win32;

namespace ReforgerModSync.App;

public partial class MainWindow : Window
{
    private readonly MainViewModel _viewModel;
    private bool _allowClose;

    public MainWindow(MainViewModel viewModel)
    {
        InitializeComponent();
        _viewModel = viewModel;
        DataContext = viewModel;
        Loaded += async (_, _) => await viewModel.InitializeAsync();

        // Ein getipptes Passwort darf nie beim nächsten Server landen.
        viewModel.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MainViewModel.SelectedProfile))
            {
                PasswordInput.Clear();
            }
        };
    }

    // Das Passwort bleibt bewusst im PasswordBox-Steuerelement und wird nie an eine Bindung gehängt.
    private async void OnSaveClick(object sender, RoutedEventArgs e)
    {
        var button = (Button)sender;
        button.IsEnabled = false;
        try
        {
            await _viewModel.SaveSelectedProfileAsync(PasswordInput.Password);
            PasswordInput.Clear();
        }
        finally
        {
            button.IsEnabled = true;
        }
    }

    private void OnDeletePasswordClick(object sender, RoutedEventArgs e)
    {
        _viewModel.DeleteSelectedPassword();
        PasswordInput.Clear();
    }

    private async void OnRemoveProfileClick(object sender, RoutedEventArgs e)
    {
        if (_viewModel.SelectedProfile is not { } profile)
        {
            return;
        }

        var answer = MessageBox.Show(
            this,
            $"Server „{profile.DisplayName}“ mit dem gespeicherten Passwort entfernen? Die geladenen Mods bleiben auf der Festplatte.",
            "Server entfernen",
            MessageBoxButton.YesNo,
            MessageBoxImage.Question,
            MessageBoxResult.No);
        if (answer == MessageBoxResult.Yes)
        {
            await _viewModel.RemoveSelectedProfileAsync();
        }
    }

    private void OnBrowseClick(object sender, RoutedEventArgs e)
    {
        if (_viewModel.SelectedProfile is not { } profile)
        {
            return;
        }

        var dialog = new OpenFolderDialog { Title = "Zielordner für die Mods wählen" };
        var current = profile.DownloadDir.Trim();
        if (Directory.Exists(current))
        {
            dialog.InitialDirectory = current;
        }

        if (dialog.ShowDialog(this) == true)
        {
            profile.DownloadDir = dialog.FolderName;
        }
    }

    private void OnStartGameClick(object sender, RoutedEventArgs e)
    {
        if (ConfirmOpenMods("Spiel starten", "Das Spiel lädt sie sonst beim Beitritt. Trotzdem starten?"))
        {
            _viewModel.StartGame();
        }
    }

    private async void OnJoinServerClick(object sender, RoutedEventArgs e)
    {
        if (ConfirmOpenMods("Direkt beitreten", "Der Beitritt kann daran scheitern. Am besten zuerst synchronisieren. Trotzdem beitreten?"))
        {
            await _viewModel.JoinServerAsync();
        }
    }

    private bool ConfirmOpenMods(string title, string question)
    {
        if (_viewModel.SelectedProfile is not { } profile)
        {
            return false;
        }

        return profile.OpenModCount == 0 || MessageBox.Show(
            this,
            $"{profile.OpenModCount} Mods passen nicht zum Server. {question}",
            title,
            MessageBoxButton.YesNo,
            MessageBoxImage.Warning,
            MessageBoxResult.No) == MessageBoxResult.Yes;
    }

    private void OnCopyLaunchOptionsClick(object sender, RoutedEventArgs e)
    {
        if (_viewModel.SelectedProfile is { } profile)
        {
            Clipboard.SetText(profile.SteamLaunchOptions);
        }
    }

    private void OnLogChanged(object sender, TextChangedEventArgs e) =>
        ((TextBox)sender).ScrollToEnd();

    /// <summary>Das Fenster wurde geschlossen, die App läuft im Infobereich weiter.</summary>
    public event Action? HiddenToTray;

    /// <summary>Beim echten Beenden das Fenster schließen lassen, statt es nur zu verstecken.</summary>
    public void AllowClose()
    {
        _allowClose = true;
        // Das Job Object beendet den Client mit, spätestens wenn dieser Prozess endet.
        _viewModel.RequestCancel();
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        if (!_allowClose)
        {
            e.Cancel = true;
            Hide();
            HiddenToTray?.Invoke();
        }

        base.OnClosing(e);
    }
}
