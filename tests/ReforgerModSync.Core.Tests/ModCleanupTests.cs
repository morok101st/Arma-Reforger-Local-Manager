using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Sync;

namespace ReforgerModSync.Core.Tests;

public class ModCleanupTests
{
    private const string IdA = "AAAAAAAAAAAAAAAA";
    private const string IdB = "BBBBBBBBBBBBBBBB";
    private const string IdC = "CCCCCCCCCCCCCCCC";
    private const string IdD = "DDDDDDDDDDDDDDDD";

    private static ModEntry Mod(string id) => new(id, $"Mod {id[0]}", "1.0.0");

    private static ServerProfile Profile(string id, string? dir) => new() { Id = id, Name = $"Server {id}", ServerAddress = "203.0.113.10:2001", DownloadDir = dir };

    private static string WriteAddon(string target, string id)
    {
        var folder = Path.Combine(target, "addons", $"Mod_{id}");
        Directory.CreateDirectory(folder);
        File.WriteAllText(Path.Combine(folder, "meta"), $$$"""
            {"meta":{"id":"{{{id}}}","name":"Mod {{{id[0]}}}","versions":[{"version":"1.0.0","package":{"files":[{"size":5,"name":"data.pak"}]}}],"selectedRev":0}}
            """);
        File.WriteAllText(Path.Combine(folder, "data.pak"), "12345");
        return folder;
    }

    private static (ModCleanup Cleanup, ModListCache Cache) Create(TempDirectory temp)
    {
        var cache = new ModListCache(Path.Combine(temp.Path, "cache"));
        return (new ModCleanup(cache), cache);
    }

    [Fact]
    public async Task Plans_only_mods_that_no_server_of_the_folder_needs()
    {
        using var temp = new TempDirectory();
        var (cleanup, cache) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdA);
        WriteAddon(target, IdB);
        WriteAddon(target, IdC);
        var surplus = WriteAddon(target, IdD);
        var current = Profile("1", target);
        var other = Profile("2", target + "\\");
        await cache.SaveAsync(other.Id, [Mod(IdB), Mod(IdC)]);

        var plan = await cleanup.PlanAsync(current, [Mod(IdA), Mod(IdB)], [current, other]);

        Assert.Null(plan.Blocker);
        var mod = Assert.Single(plan.Surplus);
        Assert.Equal(surplus, mod.Folder);
        Assert.Equal(5 + new FileInfo(Path.Combine(surplus, "meta")).Length, mod.Bytes);
    }

    [Fact]
    public async Task Server_with_another_folder_protects_nothing()
    {
        using var temp = new TempDirectory();
        var (cleanup, cache) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdA);
        var surplus = WriteAddon(target, IdB);
        var current = Profile("1", target);
        var other = Profile("2", Path.Combine(temp.Path, "andere"));
        await cache.SaveAsync(other.Id, [Mod(IdB)]);

        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current, other]);

        Assert.Equal(surplus, Assert.Single(plan.Surplus).Folder);
    }

    [Fact]
    public async Task Missing_list_of_a_server_with_the_same_folder_blocks_deleting()
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdA);
        WriteAddon(target, IdB);
        var current = Profile("1", target);
        var other = Profile("2", target.ToUpperInvariant());

        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current, other]);

        Assert.Empty(plan.Surplus);
        Assert.Contains("Server 2", plan.Blocker);
    }

    [Fact]
    public async Task Empty_list_of_another_server_blocks_deleting()
    {
        using var temp = new TempDirectory();
        var (cleanup, cache) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdB);
        var current = Profile("1", target);
        var other = Profile("2", target);
        await cache.SaveAsync(other.Id, []);

        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current, other]);

        Assert.Empty(plan.Surplus);
        Assert.NotNull(plan.Blocker);
    }

    [Fact]
    public async Task Empty_list_of_the_current_server_blocks_deleting()
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdA);
        var current = Profile("1", target);

        var plan = await cleanup.PlanAsync(current, [], [current]);

        Assert.Empty(plan.Surplus);
        Assert.NotNull(plan.Blocker);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    public async Task Default_folder_is_never_touched(string? dir)
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var current = Profile("1", dir);

        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current]);

        Assert.Empty(plan.Surplus);
        Assert.Null(plan.Blocker);
    }

    [Fact]
    public async Task Explicit_path_to_the_default_folder_counts_as_default_folder()
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var current = Profile("1", Path.Combine(AddonPaths.ClientDefaultFolder, "addons"));

        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current]);

        Assert.Empty(plan.Surplus);
    }

    [Fact]
    public async Task Folders_that_are_not_clearly_mods_stay()
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdA);
        Directory.CreateDirectory(Path.Combine(target, "addons", "saves"));
        Directory.CreateDirectory(Path.Combine(target, "addons", $"OhneMeta_{IdB}"));
        var current = Profile("1", target);

        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current]);

        Assert.Empty(plan.Surplus);
    }

    [Fact]
    public void Damaged_surplus_mod_is_selected_but_unknown_folder_is_not()
    {
        var installed = new[]
        {
            new InstalledAddon(IdA, "A", "1", "a", AddonHealth.Incomplete),
            new InstalledAddon(IdB, null, null, "b", AddonHealth.UnknownMetadata),
            new InstalledAddon(IdC, "C", "1", "c", AddonHealth.Ok),
        };

        var surplus = ModCleanup.SelectSurplus(installed, new HashSet<string>(StringComparer.OrdinalIgnoreCase) { IdC.ToLowerInvariant() });

        Assert.Equal(IdA, Assert.Single(surplus).Id);
    }

    [Fact]
    public async Task Delete_removes_only_planned_folders()
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        var keep = WriteAddon(target, IdA);
        var remove = WriteAddon(target, IdB);
        var current = Profile("1", target);
        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current]);

        var result = ModCleanup.Delete(plan);

        Assert.Single(result.Deleted);
        Assert.Empty(result.Failed);
        Assert.True(Directory.Exists(keep));
        Assert.False(Directory.Exists(remove));
    }

    [Fact]
    public async Task Locked_mod_does_not_stop_the_others()
    {
        using var temp = new TempDirectory();
        var (cleanup, _) = Create(temp);
        var target = Path.Combine(temp.Path, "mods");
        WriteAddon(target, IdA);
        var locked = WriteAddon(target, IdB);
        var free = WriteAddon(target, IdC);
        var current = Profile("1", target);
        var plan = await cleanup.PlanAsync(current, [Mod(IdA)], [current]);

        CleanupResult result;
        using (File.Open(Path.Combine(locked, "data.pak"), FileMode.Open, FileAccess.Read, FileShare.None))
        {
            result = ModCleanup.Delete(plan);
        }

        Assert.Equal(locked, Assert.Single(result.Failed).Folder);
        Assert.Equal(free, Assert.Single(result.Deleted).Folder);
    }

    [Theory]
    [InlineData(@"D:\Mods", @"d:\mods\", true)]
    [InlineData(@"D:\Mods", @"D:\Mods\addons", true)]
    [InlineData(@"D:\Mods", @"D:\Mods2", false)]
    [InlineData(null, "", true)]
    public void FolderKey_matches_equal_targets(string? left, string? right, bool same)
    {
        Assert.Equal(same, AddonPaths.FolderKey(left) == AddonPaths.FolderKey(right));
    }

    [Theory]
    [InlineData("12:00:01 SCRIPT : [RMS] LIST_DONE mods=42", 42)]
    [InlineData("[RMS] LIST_DONE mods=0", 0)]
    [InlineData("[RMS] LIST_DONE", null)]
    [InlineData("[RMS] ROOM_NOT_FOUND", null)]
    public void Reads_mod_count_of_the_list_end(string line, int? expected)
    {
        Assert.Equal(expected, ClientLogParser.ParseListDoneCount(line));
    }

    [Theory]
    [InlineData(5 * 1024 * 1024, "5")]
    [InlineData(3L * 1024 * 1024 * 1024, "3")]
    public void Formats_sizes(long bytes, string number)
    {
        var text = ModCleanup.FormatSize(bytes);
        Assert.StartsWith(number, text);
        Assert.EndsWith(bytes >= 1024L * 1024 * 1024 ? "GB" : "MB", text);
    }
}
