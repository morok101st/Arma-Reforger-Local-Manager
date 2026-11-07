using CommunityToolkit.Mvvm.ComponentModel;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.App;

/// <summary>Felder der Grundeinstellungen. Sie gelten für alle Server.</summary>
public sealed partial class BasicSettingsViewModel : ObservableObject
{
    [ObservableProperty] private string _reforgerExe = "";
    [ObservableProperty] private string _width = "";
    [ObservableProperty] private string _height = "";
    [ObservableProperty] private string _maxFps = "";
    [ObservableProperty] private string _totalMinutes = "";
    [ObservableProperty] private string _idleMinutes = "";

    [ObservableProperty] private string _reforgerExeError = "";
    [ObservableProperty] private string _windowError = "";
    [ObservableProperty] private string _maxFpsError = "";
    [ObservableProperty] private string _totalMinutesError = "";
    [ObservableProperty] private string _idleMinutesError = "";
    [ObservableProperty] private string _saveError = "";

    public BasicSettingsViewModel(BasicSettingsInput input, string? detectedExe)
    {
        Load(input);
        DetectedExeHint = detectedExe is null
            ? "Leer lassen, um den Client über Steam zu finden. Über Steam wurde gerade kein Client gefunden, dann hier eintragen."
            : $"Leer lassen, um den Client über Steam zu finden. Gefunden: {detectedExe}";
    }

    public string DetectedExeHint { get; }

    public BasicSettingsInput ToInput() => new(ReforgerExe, Width, Height, MaxFps, TotalMinutes, IdleMinutes);

    public void Load(BasicSettingsInput input)
    {
        ReforgerExe = input.ReforgerExe;
        Width = input.Width;
        Height = input.Height;
        MaxFps = input.MaxFps;
        TotalMinutes = input.TotalMinutes;
        IdleMinutes = input.IdleMinutes;
        ShowErrors(new Dictionary<BasicSettingsField, string>());
    }

    public void ShowErrors(IReadOnlyDictionary<BasicSettingsField, string> errors)
    {
        string Error(BasicSettingsField field) => errors.TryGetValue(field, out var message) ? message : "";

        ReforgerExeError = Error(BasicSettingsField.ReforgerExe);
        WindowError = string.Join(" ", new[]
        {
            errors.ContainsKey(BasicSettingsField.Width) ? $"Breite: {BasicSettingsValidator.MinWidth} bis {BasicSettingsValidator.MaxWidth}." : "",
            errors.ContainsKey(BasicSettingsField.Height) ? $"Höhe: {BasicSettingsValidator.MinHeight} bis {BasicSettingsValidator.MaxHeight}." : "",
        }.Where(text => text.Length > 0));
        MaxFpsError = Error(BasicSettingsField.MaxFps);
        TotalMinutesError = Error(BasicSettingsField.TotalMinutes);
        IdleMinutesError = Error(BasicSettingsField.IdleMinutes);
        SaveError = "";
    }
}
