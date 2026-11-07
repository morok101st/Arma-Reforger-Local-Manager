using System.IO;
using System.Windows;
using System.Windows.Input;
using Microsoft.Win32;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.App;

/// <summary>Grundeinstellungen für alle Server: Spielclient und Zeitlimits.</summary>
public partial class BasicSettingsDialog : Window
{
    private readonly BasicSettingsViewModel _fields;
    private readonly Func<BasicSettingsInput, Task<string?>> _save;

    /// <param name="save">Prüft und speichert. Liefert eine Fehlermeldung, wenn das Speichern scheitert.</param>
    private BasicSettingsDialog(Window owner, BasicSettingsInput current, string? detectedExe, Func<BasicSettingsInput, Task<string?>> save)
    {
        InitializeComponent();
        Owner = owner;
        _save = save;
        _fields = new BasicSettingsViewModel(current, detectedExe);
        DataContext = _fields;
    }

    /// <returns><c>true</c>, wenn gespeichert wurde.</returns>
    public static bool Show(Window owner, BasicSettingsInput current, string? detectedExe, Func<BasicSettingsInput, Task<string?>> save) =>
        DialogBackdrop.ShowDialog(new BasicSettingsDialog(owner, current, detectedExe, save)) == true;

    private async void OnSaveClick(object sender, RoutedEventArgs e)
    {
        var input = _fields.ToInput();
        var check = BasicSettingsValidator.Apply(new AppSettings(), input);
        _fields.ShowErrors(check.Errors);
        if (!check.IsValid)
        {
            return;
        }

        SaveButton.IsEnabled = false;
        try
        {
            if (await _save(input) is { } problem)
            {
                _fields.SaveError = problem;
                return;
            }

            DialogResult = true;
        }
        finally
        {
            SaveButton.IsEnabled = true;
        }
    }

    private void OnResetClick(object sender, RoutedEventArgs e) => _fields.Load(BasicSettingsInput.Defaults);

    private void OnBrowseClick(object sender, RoutedEventArgs e)
    {
        var dialog = new OpenFileDialog
        {
            Title = "Spielclient von Arma Reforger wählen",
            Filter = $"Arma Reforger ({SteamLocator.ClientExeName})|{SteamLocator.ClientExeName}",
        };

        var folder = Path.GetDirectoryName(_fields.ReforgerExe.Trim().Trim('"'));
        if (!string.IsNullOrEmpty(folder) && Directory.Exists(folder))
        {
            dialog.InitialDirectory = folder;
        }

        if (dialog.ShowDialog(this) == true)
        {
            _fields.ReforgerExe = dialog.FileName;
        }
    }

    private void OnNumberTextInput(object sender, TextCompositionEventArgs e) =>
        e.Handled = !e.Text.All(char.IsAsciiDigit);

    private void OnNumberPasting(object sender, DataObjectPastingEventArgs e)
    {
        if (e.DataObject.GetData(typeof(string)) is not string text || !text.Trim().All(char.IsAsciiDigit))
        {
            e.CancelCommand();
        }
    }

    private void OnDragWindow(object sender, MouseButtonEventArgs e) => DragMove();
}
