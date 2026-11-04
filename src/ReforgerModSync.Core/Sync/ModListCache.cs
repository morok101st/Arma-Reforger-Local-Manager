using System.Text.Json;
using ReforgerModSync.Core.Addons;

namespace ReforgerModSync.Core.Sync;

public sealed record CachedModList(DateTime FetchedAt, IReadOnlyList<ModEntry> Mods);

/// <summary>Merkt sich die zuletzt geholte Liste, damit die Übersicht beim App-Start ohne Spielfenster auskommt.</summary>
public sealed class ModListCache(string folder)
{
    public static string DefaultFolder { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "ReforgerModSync",
        "modlists");

    public async Task SaveAsync(string profileId, IReadOnlyList<ModEntry> mods, CancellationToken cancellationToken = default)
    {
        Directory.CreateDirectory(folder);
        var path = PathFor(profileId);
        var tempPath = path + ".tmp";
        await using (var stream = File.Create(tempPath))
        {
            await JsonSerializer.SerializeAsync(stream, new CachedModList(DateTime.Now, mods), cancellationToken: cancellationToken);
        }

        File.Move(tempPath, path, overwrite: true);
    }

    /// <summary>Eine fehlende oder beschädigte Datei ist kein Fehler, die Liste wird dann neu geholt.</summary>
    public async Task<CachedModList?> LoadAsync(string profileId, CancellationToken cancellationToken = default)
    {
        try
        {
            await using var stream = File.OpenRead(PathFor(profileId));
            return await JsonSerializer.DeserializeAsync<CachedModList>(stream, cancellationToken: cancellationToken);
        }
        catch (Exception ex) when (ex is IOException or JsonException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    public void Delete(string profileId)
    {
        File.Delete(PathFor(profileId));
    }

    private string PathFor(string profileId) => Path.Combine(folder, $"{profileId}.json");
}
