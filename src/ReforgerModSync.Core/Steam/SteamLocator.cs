using System.Diagnostics;
using System.Text.RegularExpressions;
using Microsoft.Win32;

namespace ReforgerModSync.Core.Steam;

public static partial class SteamLocator
{
    private const string ReforgerAppId = "1874880";
    private const string ClientExeName = "ArmaReforgerSteam.exe";

    public static string? ReadSteamPathFromRegistry()
    {
        using var key = Registry.CurrentUser.OpenSubKey(@"Software\Valve\Steam");
        return key?.GetValue("SteamPath") is string path && path.Length > 0 ? Path.GetFullPath(path) : null;
    }

    public static bool IsSteamRunning() => IsProcessRunning("steam");

    public static bool IsGameRunning() => IsProcessRunning(Path.GetFileNameWithoutExtension(ClientExeName));

    /// <summary>Durchsucht alle Steam-Bibliotheken nach dem Reforger-Client.</summary>
    public static string? FindClientExe(string steamPath)
    {
        foreach (var library in GetLibraryPaths(steamPath))
        {
            var steamApps = Path.Combine(library, "steamapps");
            var manifest = Path.Combine(steamApps, $"appmanifest_{ReforgerAppId}.acf");
            if (!File.Exists(manifest))
            {
                continue;
            }

            var installDir = ReadValues(File.ReadAllText(manifest), InstallDirRegex()).FirstOrDefault();
            if (installDir is null)
            {
                continue;
            }

            var exe = Path.Combine(steamApps, "common", installDir, ClientExeName);
            if (File.Exists(exe))
            {
                return exe;
            }
        }

        return null;
    }

    public static IReadOnlyList<string> GetLibraryPaths(string steamPath)
    {
        var vdf = Path.Combine(steamPath, "steamapps", "libraryfolders.vdf");
        var fromFile = File.Exists(vdf) ? ReadValues(File.ReadAllText(vdf), LibraryPathRegex()) : [];
        return new[] { steamPath }
            .Concat(fromFile)
            .Select(Path.GetFullPath)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToList();
    }

    private static IEnumerable<string> ReadValues(string vdf, Regex regex) =>
        regex.Matches(vdf).Select(match => Regex.Unescape(match.Groups["value"].Value));

    private static bool IsProcessRunning(string name)
    {
        var processes = Process.GetProcessesByName(name);
        foreach (var process in processes)
        {
            process.Dispose();
        }

        return processes.Length > 0;
    }

    [GeneratedRegex("""
        "path"\s+"(?<value>(?:[^"\\]|\\.)*)"
        """, RegexOptions.IgnoreCase)]
    private static partial Regex LibraryPathRegex();

    [GeneratedRegex("""
        "installdir"\s+"(?<value>(?:[^"\\]|\\.)*)"
        """, RegexOptions.IgnoreCase)]
    private static partial Regex InstallDirRegex();
}
