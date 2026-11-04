using System.Text.Json;

namespace ReforgerModSync.Core.Settings;

public sealed class SettingsStore(string filePath)
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        WriteIndented = true,
    };

    public static string DefaultFilePath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "ReforgerModSync",
        "settings.json");

    public string FilePath { get; } = filePath;

    /// <summary>Lädt die Einstellungen. Fehlt die Datei, gelten die Standardwerte.</summary>
    /// <exception cref="SettingsException">Die Datei ist kein gültiges JSON. Sie bleibt unverändert.</exception>
    public async Task<AppSettings> LoadAsync(CancellationToken cancellationToken = default)
    {
        if (!File.Exists(FilePath))
        {
            return new AppSettings();
        }

        await using var stream = File.OpenRead(FilePath);
        try
        {
            return await JsonSerializer.DeserializeAsync<AppSettings>(stream, JsonOptions, cancellationToken)
                ?? new AppSettings();
        }
        catch (JsonException ex)
        {
            throw new SettingsException($"Die Einstellungsdatei „{FilePath}“ ist beschädigt.", ex);
        }
    }

    /// <summary>Schreibt erst in eine Temp-Datei, damit ein Absturz die Einstellungen nicht zerstört.</summary>
    public async Task SaveAsync(AppSettings settings, CancellationToken cancellationToken = default)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(FilePath))!);
        var tempPath = FilePath + ".tmp";
        await using (var stream = File.Create(tempPath))
        {
            await JsonSerializer.SerializeAsync(stream, settings, JsonOptions, cancellationToken);
        }

        File.Move(tempPath, FilePath, overwrite: true);
    }
}

public sealed class SettingsException(string message, Exception innerException)
    : Exception(message, innerException);
