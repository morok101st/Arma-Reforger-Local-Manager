using System.Text.Json;
using System.Text.RegularExpressions;

namespace ReforgerModSync.Core.Addons;

public enum AddonHealth
{
    Ok,

    /// <summary>Eine Datei fehlt oder hat die falsche Größe, z. B. nach einem abgebrochenen Download.</summary>
    Incomplete,

    /// <summary>Der Client selbst hat die Mod als beschädigt markiert.</summary>
    Corrupted,

    /// <summary><c>meta</c> fehlt, ist unlesbar oder gehört zu einer anderen Mod.</summary>
    UnknownMetadata,
}

public sealed record InstalledAddon(string Id, string? Name, string? Version, string Folder, AddonHealth Health);

/// <summary>Liest die vom Client gepflegte Datei <c>meta</c> jedes Mod-Ordners, ohne etwas zu verändern.</summary>
public static partial class AddonScanner
{
    public static Task<IReadOnlyList<InstalledAddon>> ScanAsync(
        string modsRoot,
        CancellationToken cancellationToken = default) =>
        ScanAsync(modsRoot, null, cancellationToken);

    /// <param name="onlyIds">Nur diese Mods lesen. Spart während eines Downloads das Lesen aller übrigen Mods.</param>
    public static async Task<IReadOnlyList<InstalledAddon>> ScanAsync(
        string modsRoot,
        IReadOnlySet<string>? onlyIds,
        CancellationToken cancellationToken = default)
    {
        if (!Directory.Exists(modsRoot))
        {
            return [];
        }

        var addons = new List<InstalledAddon>();
        foreach (var folder in Directory.EnumerateDirectories(modsRoot))
        {
            var match = AddonFolderRegex().Match(Path.GetFileName(folder));
            if (match.Success && (onlyIds is null || onlyIds.Contains(match.Groups["id"].Value)))
            {
                addons.Add(await ReadAddonAsync(folder, match.Groups["id"].Value, cancellationToken));
            }
        }

        return addons;
    }

    private static async Task<InstalledAddon> ReadAddonAsync(
        string folder,
        string folderId,
        CancellationToken cancellationToken)
    {
        var unknown = new InstalledAddon(folderId, null, null, folder, AddonHealth.UnknownMetadata);
        try
        {
            // Der Client schreibt während des Downloads in diese Datei, das Tool darf ihn nie blockieren.
            await using var stream = new FileStream(
                Path.Combine(folder, "meta"),
                FileMode.Open,
                FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete);
            using var document = await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
            var meta = document.RootElement.GetProperty("meta");
            if (!string.Equals(meta.GetProperty("id").GetString(), folderId, StringComparison.OrdinalIgnoreCase))
            {
                return unknown;
            }

            var name = meta.TryGetProperty("name", out var nameElement) ? nameElement.GetString() : null;
            var selected = meta.TryGetProperty("selectedRev", out var rev) ? rev.GetInt32() : 0;
            var versions = meta.GetProperty("versions");
            if (selected < 0 || selected >= versions.GetArrayLength())
            {
                return unknown with { Name = name };
            }

            var version = versions[selected];
            return new InstalledAddon(
                folderId,
                name,
                version.GetProperty("version").GetString(),
                folder,
                CheckHealth(version, folder));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException
            or KeyNotFoundException or InvalidOperationException or FormatException)
        {
            return unknown;
        }
    }

    private static AddonHealth CheckHealth(JsonElement version, string folder)
    {
        if (version.TryGetProperty("corrupted", out var corrupted) && corrupted.GetBoolean())
        {
            return AddonHealth.Corrupted;
        }

        var files = version.GetProperty("package").GetProperty("files");
        if (files.GetArrayLength() == 0)
        {
            return AddonHealth.Incomplete;
        }

        foreach (var file in files.EnumerateArray())
        {
            if (file.TryGetProperty("isCorrupted", out var fileCorrupted) && fileCorrupted.GetBoolean())
            {
                return AddonHealth.Corrupted;
            }

            // Bewusst nicht filePath aus meta: Der Pfad ist absolut und stimmt nach einem Umzug nicht mehr.
            var name = file.GetProperty("name").GetString();
            if (string.IsNullOrEmpty(name) || Path.GetFileName(name) != name)
            {
                return AddonHealth.UnknownMetadata;
            }

            var info = new FileInfo(Path.Combine(folder, name));
            if (!info.Exists || info.Length != file.GetProperty("size").GetInt64()
                || (file.TryGetProperty("isDownloaded", out var downloaded) && !downloaded.GetBoolean()))
            {
                return AddonHealth.Incomplete;
            }
        }

        return AddonHealth.Ok;
    }

    /// <summary>Die Mod-ID aus einem Ordnernamen wie <c>Name_0123456789ABCDEF</c>, sonst <c>null</c>.</summary>
    public static string? IdOfFolder(string folder)
    {
        var match = AddonFolderRegex().Match(Path.GetFileName(folder));
        return match.Success ? match.Groups["id"].Value : null;
    }

    [GeneratedRegex("^.+_(?<id>[0-9A-Fa-f]{16})$")]
    private static partial Regex AddonFolderRegex();
}
