using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Sync;

public sealed record SurplusMod(string Name, string Folder, long Bytes);

/// <param name="FolderKey">Vergleichswert des Zielordners, siehe <see cref="AddonPaths.FolderKey"/>.</param>
/// <param name="Blocker">Warum gerade nicht abgeglichen werden kann. Dann wird nichts gelöscht.</param>
public sealed record CleanupPlan(string FolderKey, string ModsRoot, IReadOnlyList<SurplusMod> Surplus, string? Blocker = null)
{
    public long TotalBytes => Surplus.Sum(mod => mod.Bytes);
}

public sealed record CleanupResult(IReadOnlyList<SurplusMod> Deleted, IReadOnlyList<SurplusMod> Failed);

/// <summary>
/// Hält einen eigenen Zielordner deckungsgleich mit den Servern, die ihn nutzen: Gelöscht werden nur Mods, die
/// keiner dieser Server braucht. Fehlt auch nur eine Liste, wird nichts gelöscht, denn ein Fehlgriff kostet einen
/// neuen Download. Der Standardordner bleibt unberührt, dort liegen auch Mods, die das Spiel selbst geladen hat.
/// </summary>
public sealed class ModCleanup(ModListCache cache)
{
    public async Task<CleanupPlan> PlanAsync(
        ServerProfile current,
        IReadOnlyList<ModEntry> currentMods,
        IEnumerable<ServerProfile> profiles,
        CancellationToken cancellationToken = default)
    {
        var folderKey = AddonPaths.FolderKey(current.DownloadDir);
        var modsRoot = AddonPaths.ResolveModsRoot(current.DownloadDir);
        if (AddonPaths.IsDefaultFolder(current.DownloadDir))
        {
            return new CleanupPlan(folderKey, modsRoot, []);
        }

        // Eine leere Liste ist eher ein Fehler als ein Server ohne Mods und würde den ganzen Ordner leeren.
        if (currentMods.Count == 0)
        {
            return new CleanupPlan(folderKey, modsRoot, [], $"Der Server „{current.Name}“ meldet keine Mods, deshalb wird nichts gelöscht.");
        }

        var required = new HashSet<string>(currentMods.Select(mod => mod.Id), StringComparer.OrdinalIgnoreCase);
        foreach (var other in profiles.Where(profile => profile.Id != current.Id && AddonPaths.FolderKey(profile.DownloadDir) == folderKey))
        {
            if (await cache.LoadAsync(other.Id, cancellationToken) is not { Mods.Count: > 0 } list)
            {
                return new CleanupPlan(
                    folderKey,
                    modsRoot,
                    [],
                    $"Überzählige Mods wurden nicht gelöscht: Für den Server „{other.Name}“ mit demselben Zielordner fehlt die Mod-Liste. Bitte dort „Neu prüfen“ klicken.");
            }

            required.UnionWith(list.Mods.Select(mod => mod.Id));
        }

        var installed = await AddonScanner.ScanAsync(modsRoot, cancellationToken);
        var surplus = SelectSurplus(installed, required)
            .Where(addon => !IsLink(addon.Folder))
            .Select(addon => new SurplusMod(addon.Name ?? Path.GetFileName(addon.Folder), addon.Folder, SizeOf(addon.Folder)))
            .ToList();
        return new CleanupPlan(folderKey, modsRoot, surplus);
    }

    /// <summary>Ein Ordner ohne lesbares <c>meta</c> ist nicht sicher eine Mod und bleibt deshalb liegen.</summary>
    public static IReadOnlyList<InstalledAddon> SelectSurplus(IEnumerable<InstalledAddon> installed, IReadOnlySet<string> requiredIds) =>
        installed
            .Where(addon => addon.Health != AddonHealth.UnknownMetadata && !requiredIds.Contains(addon.Id))
            .OrderBy(addon => addon.Name, StringComparer.CurrentCultureIgnoreCase)
            .ToList();

    /// <summary>Eine Mod, die sich nicht löschen lässt, hält die übrigen nicht auf.</summary>
    public static CleanupResult Delete(CleanupPlan plan)
    {
        var deleted = new List<SurplusMod>();
        var failed = new List<SurplusMod>();
        foreach (var mod in plan.Surplus)
        {
            try
            {
                Directory.Delete(mod.Folder, recursive: true);
                deleted.Add(mod);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                failed.Add(mod);
            }
        }

        return new CleanupResult(deleted, failed);
    }

    public static string FormatSize(long bytes) =>
        bytes >= 1024L * 1024 * 1024
            ? $"{bytes / (1024d * 1024 * 1024):0.0} GB"
            : $"{bytes / (1024d * 1024):0.0} MB";

    private static bool IsLink(string folder) =>
        new DirectoryInfo(folder).Attributes.HasFlag(FileAttributes.ReparsePoint);

    private static long SizeOf(string folder)
    {
        var options = new EnumerationOptions { RecurseSubdirectories = true, IgnoreInaccessible = true };
        return new DirectoryInfo(folder).EnumerateFiles("*", options).Sum(file => file.Length);
    }
}
