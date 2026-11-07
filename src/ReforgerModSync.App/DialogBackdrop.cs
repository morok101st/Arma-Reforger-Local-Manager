using System.Windows;
using System.Windows.Media.Effects;

namespace ReforgerModSync.App;

/// <summary>Zeichnet das Hauptfenster hinter einem Dialog weich und dunkelt es ab.</summary>
public static class DialogBackdrop
{
    private const double BlurRadius = 8;

    /// <summary>Zeigt <paramref name="dialog"/> modal. Ohne Hauptfenster als Besitzer bleibt der Hintergrund, wie er ist.</summary>
    public static bool? ShowDialog(Window dialog)
    {
        if (dialog.Owner is not MainWindow { IsVisible: true } owner || owner.WindowContent.Effect is not null)
        {
            // Schon abgedunkelt: Ein Dialog öffnet einen zweiten, etwa eine Rückfrage.
            return dialog.ShowDialog();
        }

        owner.WindowContent.Effect = new BlurEffect { Radius = BlurRadius, RenderingBias = RenderingBias.Performance };
        owner.DialogBackdrop.Visibility = Visibility.Visible;
        try
        {
            return dialog.ShowDialog();
        }
        finally
        {
            owner.WindowContent.Effect = null;
            owner.DialogBackdrop.Visibility = Visibility.Collapsed;
        }
    }
}
