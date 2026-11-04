using System.ComponentModel;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Microsoft.Win32;

namespace ReforgerModSync.App;

public partial class MainWindow : Window
{
    private readonly MainViewModel _viewModel;

    public MainWindow(MainViewModel viewModel)
    {
        InitializeComponent();

        // Die Höhe ist so gewählt, dass die Einstellungen ohne Scrollen passen, aber nie über den Bildschirm hinaus.
        Height = Math.Min(Height, SystemParameters.WorkArea.Height);

        _viewModel = viewModel;
        DataContext = viewModel;
        Loaded += async (_, _) => await viewModel.InitializeAsync();

        // Ein getipptes Passwort darf nie beim nächsten Server landen.
        viewModel.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MainViewModel.SelectedProfile))
            {
                PasswordInput.Clear();
                if (viewModel.SelectedProfile is { } selected)
                {
                    selected.IsEditingPassword = false;
                }
            }
        };
    }

    // Das Passwort bleibt bewusst im PasswordBox-Steuerelement und wird nie an eine Bindung gehängt.
    private async void OnSaveClick(object sender, RoutedEventArgs e)
    {
        var keepPassword = false;
        if (_viewModel.NeedsPasswordTransferDecision(PasswordInput.Password))
        {
            keepPassword = BrandDialog.Confirm(
                this,
                "Server-Adresse geändert",
                "Das gespeicherte Passwort gehört zur bisherigen Adresse. Soll es auch für die neue Adresse gelten? "
                    + "Es wird dann an diesen Server übermittelt, sobald du ihn prüfst, synchronisierst oder ihm beitrittst. Ohne Übernahme wird es gelöscht.",
                "Übernehmen",
                cancelText: "Nicht übernehmen");
        }

        var button = (Button)sender;
        button.IsEnabled = false;
        try
        {
            // Nach einem Fehlschlag bleibt die Eingabe stehen, damit sie nicht neu getippt werden muss.
            var stored = await _viewModel.SaveSelectedProfileAsync(PasswordInput.Password, keepPassword);
            if (stored || _viewModel.SelectedProfile is { ShowPasswordInput: false })
            {
                PasswordInput.Clear();
            }
        }
        finally
        {
            button.IsEnabled = true;
        }
    }

    private void OnChangePasswordClick(object sender, RoutedEventArgs e)
    {
        if (_viewModel.SelectedProfile is { } profile)
        {
            profile.IsEditingPassword = true;

            // Das Feld wird gerade erst sichtbar und kann den Fokus erst nach dem Layout annehmen.
            Dispatcher.BeginInvoke(() => PasswordInput.Focus(), DispatcherPriority.Input);
        }
    }

    private void OnCancelPasswordClick(object sender, RoutedEventArgs e)
    {
        PasswordInput.Clear();
        if (_viewModel.SelectedProfile is { } profile)
        {
            profile.IsEditingPassword = false;
        }
    }

    private void OnDeletePasswordClick(object sender, RoutedEventArgs e)
    {
        var confirmed = BrandDialog.Confirm(
            this,
            "Gespeichertes Passwort löschen?",
            "Für diesen Server musst du das Passwort danach neu eingeben.",
            "Löschen",
            danger: true);
        if (confirmed)
        {
            _viewModel.DeleteSelectedPassword();
            PasswordInput.Clear();
        }
    }

    private async void OnRemoveProfileClick(object sender, RoutedEventArgs e)
    {
        if (_viewModel.SelectedProfile is not { } profile)
        {
            return;
        }

        var confirmed = BrandDialog.Confirm(
            this,
            $"Server „{profile.DisplayName}“ entfernen?",
            "Der Eintrag und sein gespeichertes Passwort werden gelöscht. Die geladenen Mods bleiben auf der Festplatte.",
            "Entfernen",
            danger: true);
        if (confirmed)
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

    private async void OnJoinServerClick(object sender, RoutedEventArgs e)
    {
        if (ConfirmOpenMods())
        {
            await _viewModel.JoinServerAsync();
        }
    }

    private bool ConfirmOpenMods()
    {
        if (_viewModel.SelectedProfile is not { } profile)
        {
            return false;
        }

        if (profile.OpenModCount == 0)
        {
            return true;
        }

        var title = profile.OpenModCount == 1
            ? "1 Mod passt nicht zum Server"
            : $"{profile.OpenModCount} Mods passen nicht zum Server";
        return BrandDialog.Confirm(this, title, "Der Beitritt kann daran scheitern. Am besten zuerst synchronisieren.", "Trotzdem beitreten");
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

    public void BringToFront()
    {
        Show();
        if (WindowState == WindowState.Minimized)
        {
            WindowState = WindowState.Normal;
        }

        Activate();
    }

    /// <summary>Schließen beendet die App. Läuft ein Sync, fragt sie vorher nach, weil er sonst abbricht.</summary>
    protected override void OnClosing(CancelEventArgs e)
    {
        if (_viewModel.IsBusy && !BrandDialog.Confirm(
                this,
                "Vorgang läuft noch",
                "Beim Beenden wird der laufende Vorgang abgebrochen. Bereits geladene Mods bleiben erhalten.",
                "Beenden"))
        {
            e.Cancel = true;
        }
        else
        {
            // Das Job Object beendet den Client mit, spätestens wenn dieser Prozess endet.
            _viewModel.RequestCancel();
        }

        base.OnClosing(e);
    }
}
