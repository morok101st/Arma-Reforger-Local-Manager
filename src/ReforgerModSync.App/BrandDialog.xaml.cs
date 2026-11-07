using System.Windows;
using System.Windows.Input;

namespace ReforgerModSync.App;

/// <summary>Rückfragen und Meldungen im Design der App statt der Windows-Standardbox.</summary>
public partial class BrandDialog : Window
{
    private BrandDialog(Window? owner, string title, string message)
    {
        InitializeComponent();
        TitleText.Text = title;
        MessageText.Text = message;

        if (owner is { IsVisible: true })
        {
            Owner = owner;
        }
        else
        {
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            ShowInTaskbar = true;
        }
    }

    /// <summary>
    /// Rückfrage. „Abbrechen“ hat den Fokus, damit ein versehentliches Enter nichts auslöst.
    /// </summary>
    /// <param name="danger">Für Aktionen, die etwas löschen: roter Rahmen statt Gold.</param>
    /// <returns><c>true</c>, wenn der Nutzer bestätigt hat.</returns>
    public static bool Confirm(Window? owner, string title, string message, string confirmText, bool danger = false, string? cancelText = null)
    {
        var dialog = new BrandDialog(owner, title, message);
        dialog.ConfirmButton.Content = confirmText;
        if (cancelText is not null)
        {
            dialog.CancelButton.Content = cancelText;
        }

        dialog.ConfirmButton.BorderBrush = (System.Windows.Media.Brush)Application.Current.Resources[danger ? "BrandDangerBrush" : "BrandGoldBrush"];
        dialog.Loaded += (_, _) => dialog.CancelButton.Focus();
        return DialogBackdrop.ShowDialog(dialog) == true;
    }

    /// <summary>Reine Meldung mit nur einer Schaltfläche.</summary>
    public static void Inform(Window? owner, string title, string message)
    {
        var dialog = new BrandDialog(owner, title, message);
        dialog.CancelButton.Content = "OK";
        dialog.ConfirmButton.Visibility = Visibility.Collapsed;
        dialog.Loaded += (_, _) => dialog.CancelButton.Focus();
        DialogBackdrop.ShowDialog(dialog);
    }

    private void OnConfirm(object sender, RoutedEventArgs e) => DialogResult = true;

    private void OnDragWindow(object sender, MouseButtonEventArgs e) => DragMove();
}
