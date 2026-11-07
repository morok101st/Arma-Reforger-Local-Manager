using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Settings;

/// <summary>Eingaben aus den Grundeinstellungen, so wie sie in den Feldern stehen.</summary>
public sealed record BasicSettingsInput(
    string ReforgerExe,
    string TotalMinutes,
    string IdleMinutes)
{
    public static BasicSettingsInput From(AppSettings settings) => new(
        settings.ReforgerExe ?? "",
        settings.Timeouts.TotalMinutes.ToString(),
        settings.Timeouts.IdleMinutes.ToString());

    public static BasicSettingsInput Defaults { get; } = From(new AppSettings());
}

public enum BasicSettingsField
{
    ReforgerExe,
    TotalMinutes,
    IdleMinutes,
}

/// <summary>Ergebnis der Prüfung: entweder neue Einstellungen oder je Feld eine Meldung.</summary>
public sealed record BasicSettingsResult(AppSettings? Settings, IReadOnlyDictionary<BasicSettingsField, string> Errors)
{
    public bool IsValid => Settings is not null;
}

/// <summary>
/// Grenzen und Prüfung der Grundeinstellungen. Sie gelten für alle Server. Das Spielfenster beim Sync
/// ist bewusst nicht dabei, seine Werte sind schon klein und bleiben, wie sie sind.
/// </summary>
public static class BasicSettingsValidator
{
    public const int MinTotalMinutes = 10, MaxTotalMinutes = 600;
    public const int MinIdleMinutes = 2, MaxIdleMinutes = 60;

    /// <summary>Übernimmt gültige Eingaben in <paramref name="current"/>. Alles andere bleibt unverändert.</summary>
    public static BasicSettingsResult Apply(AppSettings current, BasicSettingsInput input)
    {
        var errors = new Dictionary<BasicSettingsField, string>();

        var exe = input.ReforgerExe.Trim().Trim('"');
        if (exe.Length > 0 && SteamLocator.CheckClientExe(exe) is { } exeProblem)
        {
            errors[BasicSettingsField.ReforgerExe] = exeProblem;
        }

        var total = Number(input.TotalMinutes, MinTotalMinutes, MaxTotalMinutes, BasicSettingsField.TotalMinutes, errors);
        var idle = Number(input.IdleMinutes, MinIdleMinutes, MaxIdleMinutes, BasicSettingsField.IdleMinutes, errors);

        if (total is not null && idle is not null && idle >= total)
        {
            errors[BasicSettingsField.IdleMinutes] = "Muss kleiner als die Höchstdauer sein.";
        }

        if (errors.Count > 0)
        {
            return new BasicSettingsResult(null, errors);
        }

        return new BasicSettingsResult(
            current with
            {
                ReforgerExe = exe.Length == 0 ? null : Path.GetFullPath(exe),
                Timeouts = current.Timeouts with { TotalMinutes = total!.Value, IdleMinutes = idle!.Value },
            },
            errors);
    }

    private static int? Number(string text, int min, int max, BasicSettingsField field, Dictionary<BasicSettingsField, string> errors)
    {
        var trimmed = text.Trim();
        if (trimmed.Length > 0 && trimmed.All(char.IsAsciiDigit) && int.TryParse(trimmed, out var value) && value >= min && value <= max)
        {
            return value;
        }

        errors[field] = $"Bitte eine Zahl von {min} bis {max} eintragen.";
        return null;
    }
}
