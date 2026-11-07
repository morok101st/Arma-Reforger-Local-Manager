using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Sync;

public enum WatchEnd
{
    AllCurrent,
    HelperDone,
    ClientExited,
    Idle,
    TotalTimeout,
}

/// <summary>Was ein Download-Lauf überwacht.</summary>
/// <param name="Failures">Gescheiterte Mods mit Grund aus <c>[RMS] MOD_FAILED</c>. Bleibt über einen Neustart erhalten.</param>
public sealed record DownloadWatch(
    IReadOnlyList<ModEntry> Required,
    string ModsRoot,
    string LogsDir,
    TimeSpan IdleLimit,
    DateTime Deadline,
    IDictionary<string, string> Failures,
    IProgress<SyncStatus> Progress);

/// <summary>
/// Beobachtet einen laufenden Download über das Log des Clients und die <c>meta</c>-Dateien im Zielordner.
/// Gelesen werden nur die Mods, die noch nicht aktuell sind: Eine fertige Mod ändert sich nicht mehr.
/// </summary>
public sealed class DownloadWatcher(TimeSpan pollInterval, string tempAddonsDir)
{
    public static DownloadWatcher Default { get; } = new(TimeSpan.FromSeconds(3), Path.Combine(Path.GetTempPath(), "addons"));

    public async Task<WatchEnd> WatchAsync(IClientProcess client, DownloadWatch watch, CancellationToken cancellationToken)
    {
        var tail = new LogTail(Path.Combine(watch.LogsDir, "console.log"));
        var finished = new Dictionary<string, InstalledAddon>(StringComparer.OrdinalIgnoreCase);
        var lastActivity = DateTime.UtcNow;
        var lastBytes = -1L;
        var lastDone = -1;
        var helperDone = false;

        while (true)
        {
            await Task.Delay(pollInterval, cancellationToken);

            foreach (var line in tail.ReadNewLines())
            {
                var logEvent = ClientLogParser.Classify(line);
                if (logEvent == ClientLogEvent.None)
                {
                    continue;
                }

                if (logEvent != ClientLogEvent.BackendError)
                {
                    lastActivity = DateTime.UtcNow;
                }

                if (ClientLogParser.ParseModFailed(line) is { } failure)
                {
                    watch.Failures[failure.Id] = failure.Reason;
                }

                helperDone |= logEvent == ClientLogEvent.HelperDone;
                watch.Progress.Report(new SyncStatus($"[client] {line}"));
            }

            var open = watch.Required.Select(mod => mod.Id).Where(id => !finished.ContainsKey(id)).ToHashSet(StringComparer.OrdinalIgnoreCase);
            var scanned = await AddonScanner.ScanAsync(watch.ModsRoot, open, cancellationToken);
            var comparison = AddonComparer.Compare(watch.Required, scanned.Concat(finished.Values));
            foreach (var mod in comparison.Where(mod => mod.State == ModState.Current && mod.Installed is not null))
            {
                finished[mod.Required.Id] = mod.Installed!;
            }

            var done = comparison.Count(mod => mod.State == ModState.Current);
            if (done != lastDone)
            {
                lastDone = done;
                lastActivity = DateTime.UtcNow;
                watch.Progress.Report(new SyncStatus($"{done} von {comparison.Count} Mods fertig", done, comparison.Count, comparison));
            }

            if (done == comparison.Count)
            {
                return WatchEnd.AllCurrent;
            }

            if (helperDone)
            {
                return WatchEnd.HelperDone;
            }

            if (client.HasExited)
            {
                return WatchEnd.ClientExited;
            }

            // Große Mods schreiben lange keine Logzeile, wachsende Dateien zählen deshalb als Aktivität.
            var bytes = OpenModBytes(watch.ModsRoot, open) + FolderBytes(tempAddonsDir);
            if (bytes != lastBytes)
            {
                lastBytes = bytes;
                lastActivity = DateTime.UtcNow;
            }

            if (DateTime.UtcNow - lastActivity > watch.IdleLimit)
            {
                return WatchEnd.Idle;
            }

            if (DateTime.UtcNow > watch.Deadline)
            {
                return WatchEnd.TotalTimeout;
            }
        }
    }

    private static long OpenModBytes(string modsRoot, IReadOnlySet<string> openIds)
    {
        if (!Directory.Exists(modsRoot))
        {
            return 0;
        }

        return Directory.EnumerateDirectories(modsRoot)
            .Where(folder => AddonScanner.IdOfFolder(folder) is { } id && openIds.Contains(id))
            .Sum(FolderBytes);
    }

    private static long FolderBytes(string folder)
    {
        if (!Directory.Exists(folder))
        {
            return 0;
        }

        var options = new EnumerationOptions { RecurseSubdirectories = true, IgnoreInaccessible = true };
        return new DirectoryInfo(folder).EnumerateFiles("*", options).Sum(file => file.Length);
    }
}
