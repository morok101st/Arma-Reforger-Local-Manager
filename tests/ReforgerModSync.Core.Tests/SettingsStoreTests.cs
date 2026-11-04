using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Sync;

namespace ReforgerModSync.Core.Tests;

public class SettingsStoreTests
{
    [Fact]
    public async Task Missing_file_gives_no_profiles_without_creating_it()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "settings.json");

        var settings = await new SettingsStore(path).LoadAsync();

        Assert.Empty(settings.Profiles);
        Assert.False(File.Exists(path));
    }

    [Fact]
    public async Task Several_profiles_keep_their_own_values_and_the_selection()
    {
        using var temp = new TempDirectory();
        var store = new SettingsStore(Path.Combine(temp.Path, "sub", "settings.json"));
        ServerProfile first = new()
        {
            Id = "a",
            Name = "Training",
            ServerAddress = "203.0.113.10:2001",
            DownloadDir = @"D:\Mods",
        };
        ServerProfile second = new() { Id = "b", Name = "Event", ServerAddress = "203.0.113.20:2302" };

        await store.SaveAsync(new AppSettings
        {
            Profiles = [first, second],
            SelectedProfileId = "b",
            Timeouts = new TimeoutSettings { IdleMinutes = 3 },
        });
        var loaded = await store.LoadAsync();

        Assert.Equal([first, second], loaded.Profiles);
        Assert.Equal("b", loaded.SelectedProfileId);
        Assert.Equal(3, loaded.Timeouts.IdleMinutes);
    }

    [Fact]
    public async Task Passwords_cannot_end_up_in_the_settings_file()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "settings.json");

        await new SettingsStore(path).SaveAsync(new AppSettings
        {
            Profiles = [new ServerProfile { Id = "a", Name = "Training" }],
        });

        Assert.DoesNotContain("password", await File.ReadAllTextAsync(path), StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public async Task Older_settings_file_with_armm_url_still_loads()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "settings.json");
        await File.WriteAllTextAsync(path, """
            { "profiles": [ { "id": "a", "name": "Training", "exportUrl": "https://x/api/modsets/1/export", "serverAddress": "203.0.113.10:2001" } ] }
            """);

        var profile = Assert.Single((await new SettingsStore(path).LoadAsync()).Profiles);

        Assert.Equal("203.0.113.10:2001", profile.ServerAddress);
    }

    [Fact]
    public async Task Damaged_file_is_reported_and_left_untouched()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "settings.json");
        await File.WriteAllTextAsync(path, "{ kaputt");

        await Assert.ThrowsAsync<SettingsException>(() => new SettingsStore(path).LoadAsync());

        Assert.Equal("{ kaputt", await File.ReadAllTextAsync(path));
    }

    [Fact]
    public async Task Mod_list_cache_keeps_the_list_per_profile()
    {
        using var temp = new TempDirectory();
        var cache = new ModListCache(Path.Combine(temp.Path, "listen"));
        ModEntry[] mods = [new("65AD7C139EB4C1A1", "ACE Backblast Dev", "1.5.36"), new("64EE818E08AFCF94", "MFD Framework")];

        Assert.Null(await cache.LoadAsync("a"));
        await cache.SaveAsync("a", mods);

        Assert.Equal(mods, (await cache.LoadAsync("a"))!.Mods);
        Assert.Null(await cache.LoadAsync("b"));

        cache.Delete("a");
        Assert.Null(await cache.LoadAsync("a"));
    }

    [Fact]
    public async Task Damaged_cache_counts_as_not_fetched()
    {
        using var temp = new TempDirectory();
        await File.WriteAllTextAsync(Path.Combine(temp.Path, "a.json"), "{ kaputt");

        Assert.Null(await new ModListCache(temp.Path).LoadAsync("a"));
    }
}
