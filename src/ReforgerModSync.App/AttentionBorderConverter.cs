using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace ReforgerModSync.App;

/// <summary>Goldener Rahmen, wenn eine Schaltfläche gerade gebraucht wird, sonst der normale Rahmen.</summary>
public sealed class AttentionBorderConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture) =>
        value is true ? Application.Current.Resources["BrandGoldBrush"] : Application.Current.Resources["BrandBorderBrush"];

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
