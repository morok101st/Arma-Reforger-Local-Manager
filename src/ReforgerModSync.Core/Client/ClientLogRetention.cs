using System.Text.RegularExpressions;

namespace ReforgerModSync.Core.Client;

/// <summary>
/// Hält unter <c>clientlogs</c> nur die Ordner der letzten Läufe. Gelöscht werden nur Ordner,
/// deren Name ein Zeitstempel eines Laufs ist, alles andere bleibt liegen.
/// </summary>
public static partial class ClientLogRetention
{
    public const int KeepRuns = 10;

    /// <summary>Format der Ordnernamen je Lauf, sortiert sich als Text zeitlich richtig.</summary>
    public const string RunFolderFormat = "yyyyMMdd-HHmmss-fff";

    /// <summary>
    /// Löscht die ältesten Lauf-Ordner, bis höchstens <paramref name="keep"/> übrig sind. Ein gesperrter
    /// Ordner, etwa weil ein Client noch schreibt, bleibt liegen und wird beim nächsten Lauf erneut versucht.
    /// </summary>
    /// <returns>Anzahl der gelöschten Ordner.</returns>
    public static int Prune(string clientLogsRoot, int keep = KeepRuns)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(keep);
        if (!Directory.Exists(clientLogsRoot))
        {
            return 0;
        }

        var runs = Directory.EnumerateDirectories(clientLogsRoot)
            .Where(path => RunFolderRegex().IsMatch(Path.GetFileName(path)))
            .Order(StringComparer.Ordinal)
            .ToList();

        var deleted = 0;
        foreach (var run in runs.Take(Math.Max(0, runs.Count - keep)))
        {
            try
            {
                Directory.Delete(run, recursive: true);
                deleted++;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // Bleibt liegen, der Sync soll daran nicht scheitern.
            }
        }

        return deleted;
    }

    /// <summary>Legt den Log-Ordner für einen neuen Lauf an und lässt danach genau <see cref="KeepRuns"/> Läufe übrig.</summary>
    public static string CreateRunFolder(string clientLogsRoot)
    {
        Prune(clientLogsRoot, KeepRuns - 1);
        var folder = Path.Combine(clientLogsRoot, DateTime.Now.ToString(RunFolderFormat));
        Directory.CreateDirectory(folder);
        return folder;
    }

    [GeneratedRegex(@"^\d{8}-\d{6}-\d{3}$")]
    private static partial Regex RunFolderRegex();
}
