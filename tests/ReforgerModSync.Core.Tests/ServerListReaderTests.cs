using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Sync;

namespace ReforgerModSync.Core.Tests;

public class ServerListReaderTests
{
    private static readonly ServerListReader Reader = new(TimeSpan.FromMilliseconds(10), TimeSpan.FromSeconds(5));

    private static Task<IReadOnlyList<ModEntry>> ReadSampleAsync(TempDirectory temp, string sample, FakeClient? client = null)
    {
        FakeClient.WriteLog(temp.Path, FakeClient.Sample(sample));
        return Reader.ReadAsync(client ?? new FakeClient(), temp.Path, "203.0.113.10:2001", CancellationToken.None);
    }

    [Fact]
    public async Task Reads_all_mods_including_one_without_version()
    {
        using var temp = new TempDirectory();

        var mods = await ReadSampleAsync(temp, "list-done.log");

        Assert.Equal(
            [
                new ModEntry("1A2B3C4D5E6F7001", "Beispiel Fahrzeuge", "1.4.0"),
                new ModEntry("1A2B3C4D5E6F7002", "Beispiel Karte Nord", "2.1.3"),
                new ModEntry("1A2B3C4D5E6F7003", "Beispiel ohne Version", null),
            ],
            mods);
    }

    [Fact]
    public async Task An_incomplete_list_never_counts()
    {
        using var temp = new TempDirectory();

        var error = await Assert.ThrowsAsync<ServerListException>(() => ReadSampleAsync(temp, "list-incomplete.log"));

        Assert.StartsWith("Die Mod-Liste kam unvollständig an (1 von 2 Mods)", error.Message);
    }

    [Fact]
    public async Task Missing_password_gets_a_clear_message()
    {
        using var temp = new TempDirectory();

        var error = await Assert.ThrowsAsync<ServerListException>(() => ReadSampleAsync(temp, "list-password-required.log"));

        Assert.Equal("Der Server verlangt ein Passwort. Bitte in den Einstellungen eintragen.", error.Message);
    }

    [Fact]
    public async Task A_room_with_another_address_counts_as_not_found()
    {
        using var temp = new TempDirectory();

        var error = await Assert.ThrowsAsync<ServerListException>(() => ReadSampleAsync(temp, "list-address-mismatch.log"));

        Assert.StartsWith("Der Server 203.0.113.10:2001 wurde nicht gefunden.", error.Message);
    }

    [Fact]
    public async Task Lines_written_while_reading_are_picked_up()
    {
        using var temp = new TempDirectory();
        var lines = FakeClient.Sample("list-done.log").Split('\n');
        FakeClient.WriteLog(temp.Path, string.Join('\n', lines.Take(7)) + "\n");

        var reading = Reader.ReadAsync(new FakeClient(), temp.Path, "203.0.113.10:2001", CancellationToken.None);
        await Task.Delay(50);
        File.AppendAllText(Path.Combine(temp.Path, "console.log"), string.Join('\n', lines.Skip(7)));

        Assert.Equal(3, (await reading).Count);
    }

    [Fact]
    public async Task A_client_that_exits_early_is_reported_with_its_logs()
    {
        using var temp = new TempDirectory();
        FakeClient.WriteLog(temp.Path, "14:53:09.618 ENGINE       : Game successfully created.\n");
        File.WriteAllText(Path.Combine(temp.Path, "crash.log"), "");

        var error = await Assert.ThrowsAsync<ServerListException>(() =>
            Reader.ReadAsync(new FakeClient { HasExited = true }, temp.Path, "203.0.113.10:2001", CancellationToken.None));

        Assert.Equal($"Der Spielclient ist abgestürzt. Client-Logs: {temp.Path}", error.Message);
    }

    [Fact]
    public async Task Gives_up_after_the_timeout()
    {
        using var temp = new TempDirectory();
        var reader = new ServerListReader(TimeSpan.FromMilliseconds(10), TimeSpan.FromMilliseconds(100));

        var error = await Assert.ThrowsAsync<ServerListException>(() =>
            reader.ReadAsync(new FakeClient(), temp.Path, "203.0.113.10:2001", CancellationToken.None));

        Assert.StartsWith("Die Mod-Liste kam nicht innerhalb von", error.Message);
    }

    [Fact]
    public async Task Cancelling_stops_reading()
    {
        using var temp = new TempDirectory();
        using var cancel = new CancellationTokenSource(TimeSpan.FromMilliseconds(50));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            Reader.ReadAsync(new FakeClient(), temp.Path, "203.0.113.10:2001", cancel.Token));
    }
}
