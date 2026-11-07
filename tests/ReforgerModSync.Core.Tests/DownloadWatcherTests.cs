using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Sync;

namespace ReforgerModSync.Core.Tests;

public class DownloadWatcherTests
{
    private const string IdA = "1A2B3C4D5E6F7001";
    private const string IdB = "1A2B3C4D5E6F7002";

    private static readonly IReadOnlyList<ModEntry> Required =
    [
        new(IdA, "Beispiel Fahrzeuge", "1.4.0"),
        new(IdB, "Beispiel Karte Nord", "2.1.3"),
    ];

    private sealed class Setup : IDisposable
    {
        private readonly TempDirectory _temp = new();

        public Setup()
        {
            Directory.CreateDirectory(ModsRoot);
            Directory.CreateDirectory(LogsDir);
            Directory.CreateDirectory(TempAddons);
        }

        public string ModsRoot => Path.Combine(_temp.Path, "mods", "addons");
        public string LogsDir => Path.Combine(_temp.Path, "logs");
        public string TempAddons => Path.Combine(_temp.Path, "tempaddons");
        public List<SyncStatus> Reports { get; } = [];
        public Dictionary<string, string> Failures { get; } = new(StringComparer.OrdinalIgnoreCase);

        public DownloadWatcher Watcher => new(TimeSpan.FromMilliseconds(10), TempAddons);

        public DownloadWatch Watch(TimeSpan? idle = null, DateTime? deadline = null) => new(
            Required,
            ModsRoot,
            LogsDir,
            idle ?? TimeSpan.FromSeconds(30),
            deadline ?? DateTime.UtcNow.AddMinutes(5),
            Failures,
            new SyncProgress(Reports));

        public void WriteAddon(string id, string version)
        {
            var folder = Path.Combine(ModsRoot, $"Mod_{id}");
            Directory.CreateDirectory(folder);
            File.WriteAllText(Path.Combine(folder, "data.pak"), "12345");
            File.WriteAllText(Path.Combine(folder, "meta"), $$$"""
                {"meta":{"id":"{{{id}}}","name":"Mod","versions":[{"version":"{{{version}}}","package":{"files":[{"size":5,"name":"data.pak"}]}}],"selectedRev":0}}
                """);
        }

        public void Dispose() => _temp.Dispose();
    }

    /// <summary>Meldet synchron, anders als <see cref="Progress{T}"/>, damit Tests die Meldungen sofort sehen.</summary>
    private sealed class SyncProgress(List<SyncStatus> reports) : IProgress<SyncStatus>
    {
        public void Report(SyncStatus value)
        {
            lock (reports)
            {
                reports.Add(value);
            }
        }
    }

    [Fact]
    public async Task Ends_when_all_mods_are_current()
    {
        using var setup = new Setup();
        setup.WriteAddon(IdA, "1.4.0");
        var watching = setup.Watcher.WatchAsync(new FakeClient(), setup.Watch(), CancellationToken.None);

        await Task.Delay(50);
        setup.WriteAddon(IdB, "2.1.3");

        Assert.Equal(WatchEnd.AllCurrent, await watching);
        Assert.Contains(setup.Reports, status => status is { Message: "2 von 2 Mods fertig", Done: 2, Total: 2 });
    }

    [Fact]
    public async Task Helper_done_ends_the_watch_and_failures_are_kept()
    {
        using var setup = new Setup();
        setup.WriteAddon(IdA, "1.4.0");
        FakeClient.WriteLog(setup.LogsDir, FakeClient.Sample("download.log"));

        var end = await setup.Watcher.WatchAsync(new FakeClient(), setup.Watch(), CancellationToken.None);

        Assert.Equal(WatchEnd.HelperDone, end);
        Assert.Equal("download failed", setup.Failures[IdB]);
        Assert.Contains(setup.Reports, status => status.Message.EndsWith("[RMS] MOD_DONE 1A2B3C4D5E6F7001 1.4.0"));
    }

    [Fact]
    public async Task An_outdated_mod_is_not_yet_finished()
    {
        using var setup = new Setup();
        setup.WriteAddon(IdA, "1.4.0");
        setup.WriteAddon(IdB, "2.0.0");

        var end = await setup.Watcher.WatchAsync(new FakeClient { HasExited = true }, setup.Watch(), CancellationToken.None);

        Assert.Equal(WatchEnd.ClientExited, end);
        Assert.Contains(setup.Reports, status => status is { Done: 1, Total: 2 });
    }

    [Fact]
    public async Task Without_any_activity_the_watch_reports_idle()
    {
        using var setup = new Setup();

        var end = await setup.Watcher.WatchAsync(new FakeClient(), setup.Watch(idle: TimeSpan.FromMilliseconds(100)), CancellationToken.None);

        Assert.Equal(WatchEnd.Idle, end);
    }

    [Fact]
    public async Task Growing_files_count_as_activity()
    {
        using var setup = new Setup();
        var watch = setup.Watch(idle: TimeSpan.FromMilliseconds(300), deadline: DateTime.UtcNow.AddMilliseconds(900));
        var watching = setup.Watcher.WatchAsync(new FakeClient(), watch, CancellationToken.None);

        // Ohne Logzeile, nur ein wachsender Download im Temp-Ordner: Das darf nicht als Stillstand gelten.
        var part = Path.Combine(setup.TempAddons, "part.tmp");
        for (var i = 0; i < 9; i++)
        {
            File.AppendAllText(part, new string('x', 100));
            await Task.Delay(100);
        }

        Assert.Equal(WatchEnd.TotalTimeout, await watching);
    }

    [Fact]
    public async Task Cancelling_stops_the_watch()
    {
        using var setup = new Setup();
        using var cancel = new CancellationTokenSource(TimeSpan.FromMilliseconds(50));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            setup.Watcher.WatchAsync(new FakeClient(), setup.Watch(), cancel.Token));
    }
}
