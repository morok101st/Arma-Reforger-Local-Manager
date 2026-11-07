namespace ReforgerModSync.Core.Settings;

/// <summary>
/// Macht eine geladene Einstellungsdatei sicher benutzbar. Die Datei kann von Hand geändert oder beschädigt
/// sein: Fehlende Abschnitte und Werte außerhalb der Grenzen fallen auf den Standard zurück. Ein ungültiger
/// Spielclient bleibt stehen, damit die Vorprüfung ihn meldet, statt still einen anderen zu nehmen.
/// </summary>
public static class AppSettingsSanitizer
{
    public const int MinWidth = 320, MaxWidth = 3840;
    public const int MinHeight = 180, MaxHeight = 2160;
    public const int MinFps = 5, MaxFps = 60;

    public static AppSettings Sanitize(AppSettings settings)
    {
        var defaults = new AppSettings();
        var client = settings.Client ?? defaults.Client;
        var timeouts = settings.Timeouts ?? defaults.Timeouts;

        var total = InRange(timeouts.TotalMinutes, BasicSettingsValidator.MinTotalMinutes, BasicSettingsValidator.MaxTotalMinutes)
            ? timeouts.TotalMinutes
            : defaults.Timeouts.TotalMinutes;
        var idle = InRange(timeouts.IdleMinutes, BasicSettingsValidator.MinIdleMinutes, BasicSettingsValidator.MaxIdleMinutes)
            ? timeouts.IdleMinutes
            : defaults.Timeouts.IdleMinutes;
        if (idle >= total)
        {
            (total, idle) = (defaults.Timeouts.TotalMinutes, defaults.Timeouts.IdleMinutes);
        }

        return settings with
        {
            // Ohne ID hinge das Passwort an keinem eindeutigen Eintrag.
            Profiles = (settings.Profiles ?? []).Where(profile => profile is not null && !string.IsNullOrWhiteSpace(profile.Id)).ToList(),
            CleanupApprovedFolders = (settings.CleanupApprovedFolders ?? []).Where(folder => !string.IsNullOrWhiteSpace(folder)).ToList(),
            ReforgerExe = string.IsNullOrWhiteSpace(settings.ReforgerExe) ? null : settings.ReforgerExe.Trim(),
            Client = client with
            {
                Width = InRange(client.Width, MinWidth, MaxWidth) ? client.Width : defaults.Client.Width,
                Height = InRange(client.Height, MinHeight, MaxHeight) ? client.Height : defaults.Client.Height,
                MaxFps = InRange(client.MaxFps, MinFps, MaxFps) ? client.MaxFps : defaults.Client.MaxFps,
            },
            Timeouts = timeouts with { TotalMinutes = total, IdleMinutes = idle },
        };
    }

    private static bool InRange(int value, int min, int max) => value >= min && value <= max;
}
