namespace ReforgerModSync.Core.Addons;

public static class AddonPaths
{
    /// <summary>
    /// Der Client legt die Mods in <c>&lt;Zielordner&gt;\addons\addons</c> ab. Nutzer wählen oft direkt den Ordner mit
    /// den Mods. Diese Endung wird entfernt, damit die App sie nicht doppelt anhängt.
    /// </summary>
    public static string? NormalizeDownloadDir(string? downloadDir)
    {
        if (string.IsNullOrWhiteSpace(downloadDir))
        {
            return null;
        }

        var path = downloadDir.Trim().TrimEnd('\\', '/');
        foreach (var _ in new[] { 1, 2 })
        {
            if (string.Equals(Path.GetFileName(path), "addons", StringComparison.OrdinalIgnoreCase)
                && Path.GetDirectoryName(path) is { Length: > 0 } parent)
            {
                path = parent.TrimEnd('\\', '/');
            }
        }

        return path.EndsWith(':') ? path + '\\' : path;
    }

    /// <summary>Basisordner des Clients, wenn der Nutzer keinen Zielordner gewählt hat.</summary>
    public static string ClientDefaultFolder { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        "My Games",
        "ArmaReforger");

    /// <summary>Ordner, der die mitgelieferte Hilfs-Mod enthält.</summary>
    public static string HelperRoot { get; } = Path.Combine(AppContext.BaseDirectory, "helper");

    /// <summary>Ordner, in dem die Mod-Ordner liegen. Beobachtet an einer echten Installation.</summary>
    public static string ResolveModsRoot(string? downloadDir) =>
        NormalizeDownloadDir(downloadDir) is { } folder
            ? Path.Combine(folder, "addons", "addons")
            : Path.Combine(ClientDefaultFolder, "addons");
}
