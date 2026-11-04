using System.Text.RegularExpressions;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Client;

public static partial class ClientArgsBuilder
{
    public const string HelperGuid = "7A3C1E5D9B2F4860";

    /// <summary>Startparameter, mit denen die Hilfs-Mod genau diese Mods lädt.</summary>
    public static IReadOnlyList<string> BuildDownload(
        IEnumerable<ModEntry> mods,
        string? downloadDir,
        ClientWindowSettings window,
        string logsDir,
        string profileDir,
        string helperRoot) =>
        [
            .. BuildCommon(downloadDir, window, logsDir, profileDir, helperRoot),
            "-rmsMods", FormatMods(mods),
        ];

    /// <summary>Startparameter, mit denen die Hilfs-Mod die Mod-Liste eines Servers abruft. Das Passwort steht nie darin.</summary>
    public static IReadOnlyList<string> BuildListRequest(
        string serverAddress,
        string? downloadDir,
        ClientWindowSettings window,
        string logsDir,
        string profileDir,
        string helperRoot) =>
        [
            .. BuildCommon(downloadDir, window, logsDir, profileDir, helperRoot),
            "-rmsServer", serverAddress.Trim(),
        ];

    /// <summary>
    /// Startparameter für den direkten Beitritt: mit Hilfs-Mod und echtem Profil des Spielers. Beim
    /// Verbinden lädt das Spiel die Mods des Servers neu. Das Passwort steht nie darin.
    /// </summary>
    public static IReadOnlyList<string> BuildJoin(string serverAddress, string? downloadDir, string helperRoot)
    {
        var folder = AddonPaths.NormalizeDownloadDir(downloadDir);
        return
        [
            "-addonsDir", folder is null ? helperRoot : $"{folder},{helperRoot}",
            .. folder is null ? Array.Empty<string>() : ["-addonDownloadDir", folder],
            "-addons", HelperGuid,
            "-rmsJoin", serverAddress.Trim(),
        ];
    }

    /// <summary>Format der Hilfs-Mod: <c>id:version;id:version</c>, ohne Version die neueste.</summary>
    public static string FormatMods(IEnumerable<ModEntry> mods) =>
        string.Join(';', mods.Select(mod => mod.Version is null ? mod.Id : $"{mod.Id}:{mod.Version}"));

    /// <summary>Text für die Steam-Startoptionen, damit das Spiel denselben Ordner nutzt.</summary>
    public static string SteamLaunchOptions(string? downloadDir) =>
        AddonPaths.NormalizeDownloadDir(downloadDir) is { } folder
            ? $"-addonsDir \"{folder}\" -addonDownloadDir \"{folder}\""
            : "";

    /// <summary>Baut eine Befehlszeile, Werte mit Leerzeichen kommen in Anführungszeichen.</summary>
    public static string ToCommandLine(IEnumerable<string> arguments) =>
        string.Join(' ', arguments.Select(argument => argument.Contains(' ') ? $"\"{argument}\"" : argument));

    /// <summary>Nur IP und Port: Ein Hostname wird im Backend nicht gefunden.</summary>
    public static bool IsValidServerAddress(string? address) =>
        address is not null && ServerAddressRegex().IsMatch(address.Trim());

    /// <summary>
    /// Ein eigenes Profil lässt die Einstellungen des Spielers unberührt, ändert aber auch den
    /// Standard-Mod-Ordner des Clients. Der Download-Ordner wird deshalb immer ausdrücklich gesetzt.
    /// </summary>
    private static IEnumerable<string> BuildCommon(
        string? downloadDir,
        ClientWindowSettings window,
        string logsDir,
        string profileDir,
        string helperRoot)
    {
        var folder = AddonPaths.NormalizeDownloadDir(downloadDir) ?? AddonPaths.ClientDefaultFolder;

        return
        [
            "-window",
            "-screenWidth", window.Width.ToString(),
            "-screenHeight", window.Height.ToString(),
            "-noSound",
            "-noSplash",
            "-noFocus",
            "-maxFPS", window.MaxFps.ToString(),
            "-addonsDir", $"{folder},{helperRoot}",
            "-addonDownloadDir", folder,
            "-addons", HelperGuid,
            "-profile", profileDir,
            "-logsDir", logsDir,
        ];
    }

    [GeneratedRegex(@"^(\d{1,3}\.){3}\d{1,3}:\d{1,5}$")]
    private static partial Regex ServerAddressRegex();
}
