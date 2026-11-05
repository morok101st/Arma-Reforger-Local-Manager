namespace ReforgerModSync.Core.Addons;

public static class AddonPaths
{
    /// <summary>
    /// Der Client legt die Mods in <c>&lt;Zielordner&gt;\addons</c> ab. Nutzer wählen oft direkt den Ordner mit
    /// den Mods. Diese Endung wird entfernt, damit der Client sie nicht doppelt anhängt.
    /// </summary>
    public static string? NormalizeDownloadDir(string? downloadDir)
    {
        if (string.IsNullOrWhiteSpace(downloadDir))
        {
            return null;
        }

        var path = downloadDir.Trim().TrimEnd('\\', '/');
        if (string.Equals(Path.GetFileName(path), "addons", StringComparison.OrdinalIgnoreCase)
            && Path.GetDirectoryName(path) is { Length: > 0 } parent)
        {
            path = parent.TrimEnd('\\', '/');
        }

        return path.EndsWith(':') ? path + '\\' : path;
    }

    /// <summary>Basisordner des Clients, wenn der Nutzer keinen Zielordner gewählt hat.</summary>
    public static string ClientDefaultFolder { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        "My Games",
        "ArmaReforger");

    /// <summary>Vergleichswert: Gleich, wenn zwei Angaben denselben Zielordner meinen, auch über den Standardordner.</summary>
    public static string FolderKey(string? downloadDir) =>
        Path.TrimEndingDirectorySeparator(Path.GetFullPath(NormalizeDownloadDir(downloadDir) ?? ClientDefaultFolder))
            .ToUpperInvariant();

    public static bool IsDefaultFolder(string? downloadDir) => FolderKey(downloadDir) == FolderKey(null);

    /// <summary>Ordner, der die mitgelieferte Hilfs-Mod enthält.</summary>
    public static string HelperRoot { get; } = Path.Combine(AppContext.BaseDirectory, "helper");

    /// <summary>Ordner, in dem die Mod-Ordner liegen. Beobachtet an einer echten Installation.</summary>
    public static string ResolveModsRoot(string? downloadDir) =>
        Path.Combine(NormalizeDownloadDir(downloadDir) ?? ClientDefaultFolder, "addons");
}
