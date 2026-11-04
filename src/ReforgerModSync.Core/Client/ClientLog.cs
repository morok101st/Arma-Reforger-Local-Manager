using System.Text;
using System.Text.RegularExpressions;
using ReforgerModSync.Core.Addons;

namespace ReforgerModSync.Core.Client;

public enum ServerListEnd
{
    Done,
    NotFound,
    PasswordRequired,
    PasswordRejected,
    Failed,
}

public enum ClientLogEvent
{
    None,
    Activity,
    BackendError,
    ModFailed,
    HelperDone,
}

/// <summary>Muster stammen aus echten Client-Logs und den [RMS]-Zeilen der Hilfs-Mod.</summary>
public static partial class ClientLogParser
{
    public static ClientLogEvent Classify(string line)
    {
        if (HelperDoneRegex().IsMatch(line))
        {
            return ClientLogEvent.HelperDone;
        }

        if (ModFailedRegex().IsMatch(line))
        {
            return ClientLogEvent.ModFailed;
        }

        if (line.Contains("[RMS] ", StringComparison.Ordinal) || BackendDownloadRegex().IsMatch(line))
        {
            return ClientLogEvent.Activity;
        }

        return BackendErrorRegex().IsMatch(line) ? ClientLogEvent.BackendError : ClientLogEvent.None;
    }

    /// <summary>Liefert die ID und den Grund einer Zeile <c>[RMS] MOD_FAILED id grund</c>.</summary>
    public static (string Id, string Reason)? ParseModFailed(string line)
    {
        var match = ModFailedRegex().Match(line);
        return match.Success ? (match.Groups["id"].Value, match.Groups["reason"].Value.Trim()) : null;
    }

    /// <summary>Liest eine Zeile <c>[RMS] SERVER_MOD id version name</c>. Die Version kann leer sein.</summary>
    public static ModEntry? ParseServerMod(string line)
    {
        var match = ServerModRegex().Match(line);
        if (!match.Success)
        {
            return null;
        }

        var version = match.Groups["version"].Value;
        return new ModEntry(match.Groups["id"].Value, match.Groups["name"].Value.Trim(), version.Length == 0 ? null : version);
    }

    /// <summary>Wertet das Endergebnis eines Listenabrufs aus. <c>null</c>: noch kein Ergebnis in dieser Zeile.</summary>
    public static ServerListEnd? ParseServerListEnd(string line)
    {
        var match = ServerListEndRegex().Match(line);
        if (!match.Success)
        {
            return null;
        }

        return match.Groups["end"].Value switch
        {
            "LIST_DONE" => ServerListEnd.Done,
            "ROOM_NOT_FOUND" => ServerListEnd.NotFound,
            "ROOM_PASSWORD_REQUIRED" => ServerListEnd.PasswordRequired,
            "ROOM_PASSWORD_REJECTED" => ServerListEnd.PasswordRejected,
            _ => ServerListEnd.Failed,
        };
    }

    [GeneratedRegex(@"\[RMS\] SERVER_MOD (?<id>[0-9A-Fa-f]{16}) (?<version>\S*) (?<name>.*)$")]
    private static partial Regex ServerModRegex();

    [GeneratedRegex(@"\[RMS\] (?<end>LIST_DONE|ROOM_NOT_FOUND|ROOM_PASSWORD_REQUIRED|ROOM_PASSWORD_REJECTED|ROOM_SEARCH_FAILED|ROOM_LIST_FAILED)\b")]
    private static partial Regex ServerListEndRegex();

    [GeneratedRegex(@"\[RMS\] DONE\b")]
    private static partial Regex HelperDoneRegex();

    [GeneratedRegex(@"\[RMS\] MOD_FAILED (?<id>[0-9A-Fa-f]{16})(?<reason>.*)$")]
    private static partial Regex ModFailedRegex();

    [GeneratedRegex(@"BACKEND\s*: (Addon Download started|Downloading|Updating) ")]
    private static partial Regex BackendDownloadRegex();

    [GeneratedRegex(@"BACKEND\s*\(E\):")]
    private static partial Regex BackendErrorRegex();
}

/// <summary>Liest neue vollständige Zeilen einer Datei, die der Client gerade schreibt.</summary>
public sealed class LogTail(string filePath)
{
    private long _position;
    private string _partialLine = "";

    public IReadOnlyList<string> ReadNewLines()
    {
        if (!File.Exists(filePath))
        {
            return [];
        }

        using var stream = new FileStream(filePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        if (stream.Length <= _position)
        {
            return [];
        }

        stream.Position = _position;
        var buffer = new byte[stream.Length - _position];
        stream.ReadExactly(buffer);
        _position = stream.Length;

        var parts = (_partialLine + Encoding.UTF8.GetString(buffer)).Split('\n');
        _partialLine = parts[^1];
        return parts[..^1].Select(line => line.TrimEnd('\r')).ToList();
    }
}
