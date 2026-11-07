using System.Text.RegularExpressions;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Client;

public static partial class ClientArgsBuilder
{
    public const string HelperGuid = "7A3C1E5D9B2F4860";

    /// <summary>
    /// Startparameter, mit denen die Hilfs-Mod genau diese Mods in der Version des Servers lädt. Sie braucht
    /// dafür den Server-Eintrag, die Workshop-Suche findet manche Mods nicht. Das Passwort steht nie darin.
    /// </summary>
    public static IReadOnlyList<string> BuildDownload(
        string serverAddress,
        IEnumerable<ModEntry> mods,
        string? downloadDir,
        ClientWindowSettings window,
        string logsDir,
        string profileDir,
        string helperRoot) =>
        [
            .. BuildCommon(downloadDir, window, logsDir, profileDir, helperRoot),
            "-rmsServer", serverAddress.Trim(),
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
    /// <param name="logsDir">Log-Ordner der App. Dort liegt auch die Passwortdatei, nicht in „Dokumente“, das oft in OneDrive liegt.</param>
    public static IReadOnlyList<string> BuildJoin(string serverAddress, string? downloadDir, string helperRoot, string logsDir)
    {
        var folder = AddonPaths.NormalizeDownloadDir(downloadDir);
        return
        [
            "-addonsDir", folder is null ? helperRoot : $"{folder},{helperRoot}",
            .. folder is null ? Array.Empty<string>() : ["-addonDownloadDir", folder],
            "-addons", HelperGuid,
            "-logsDir", logsDir,
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
    public static bool IsValidServerAddress(string? address)
    {
        var (ip, port) = SplitServerAddress(address);
        return IsValidIp(ip) && IsValidPort(port);
    }

    public static bool IsValidIp(string? ip) =>
        ip is not null && IpRegex().IsMatch(ip.Trim());

    public static bool IsValidPort(string? port) =>
        int.TryParse(port?.Trim(), out var number) && number is >= 1 and <= 65535;

    /// <summary>Zerlegt <c>IP:Port</c> für die Eingabefelder. Ohne Doppelpunkt ist der Port leer.</summary>
    public static (string Ip, string Port) SplitServerAddress(string? address)
    {
        var text = address?.Trim() ?? "";
        var separator = text.LastIndexOf(':');
        return separator < 0 ? (text, "") : (text[..separator].Trim(), text[(separator + 1)..].Trim());
    }

    /// <summary>Gespeichert wird immer <c>IP:Port</c>, daran hängt auch das Passwort. <c>null</c>: beide Felder leer.</summary>
    public static string? CombineServerAddress(string? ip, string? port)
    {
        var cleanIp = ip?.Trim() ?? "";
        var cleanPort = port?.Trim() ?? "";
        if (cleanIp.Length == 0 && cleanPort.Length == 0)
        {
            return null;
        }

        return $"{cleanIp}:{(int.TryParse(cleanPort, out var number) ? number.ToString() : cleanPort)}";
    }

    /// <summary>Lässt im Portfeld nur Ziffern zu, auch beim Einfügen, und höchstens die fünf eines gültigen Ports.</summary>
    public static string CleanPortInput(string? input)
    {
        var digits = new string((input ?? "").Where(char.IsAsciiDigit).ToArray());
        return digits.Length > 5 ? digits[..5] : digits;
    }

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

    [GeneratedRegex(@"^((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$")]
    private static partial Regex IpRegex();
}
