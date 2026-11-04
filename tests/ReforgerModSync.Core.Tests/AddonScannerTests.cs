using ReforgerModSync.Core.Addons;

namespace ReforgerModSync.Core.Tests;

public class AddonScannerTests
{
    private const string Id = "660E395AD366D1F7";

    private static string WriteAddon(
        string root,
        string id = Id,
        string fileContent = "12345",
        int metaSize = 5,
        bool corrupted = false,
        bool fileOnDisk = true)
    {
        var folder = Path.Combine(root, $"Test-Mod_{id}");
        Directory.CreateDirectory(folder);
        // Aufbau wie in echten meta-Dateien des Clients, inklusive veraltetem absolutem filePath.
        File.WriteAllText(Path.Combine(folder, "meta"), $$$"""
            {"meta":{"id":"{{{id}}}","name":"Test Mod","versions":[
              {"version":"9.9.9","package":{"files":[]}},
              {"version":"0.2.4","corrupted":{{{corrupted.ToString().ToLowerInvariant()}}},"package":{"totalSize":{{{metaSize}}},"files":[
                {"isCorrupted":false,"isDownloaded":true,"size":{{{metaSize}}},"filePath":"Z:\\weg\\data.pak","name":"data.pak"}]}}
            ],"selectedRev":1}}
            """);
        if (fileOnDisk)
        {
            File.WriteAllText(Path.Combine(folder, "data.pak"), fileContent);
        }

        return folder;
    }

    [Fact]
    public async Task Reads_id_name_and_selected_version()
    {
        using var temp = new TempDirectory();
        var folder = WriteAddon(temp.Path);

        var addon = Assert.Single(await AddonScanner.ScanAsync(temp.Path));

        Assert.Equal(new InstalledAddon(Id, "Test Mod", "0.2.4", folder, AddonHealth.Ok), addon);
    }

    [Fact]
    public async Task Skips_folders_without_mod_id_like_saves()
    {
        using var temp = new TempDirectory();
        Directory.CreateDirectory(Path.Combine(temp.Path, "saves"));

        Assert.Empty(await AddonScanner.ScanAsync(temp.Path));
    }

    [Fact]
    public async Task Missing_root_gives_empty_result() =>
        Assert.Empty(await AddonScanner.ScanAsync(@"Z:\gibt\es\nicht"));

    [Fact]
    public async Task Missing_file_means_incomplete()
    {
        using var temp = new TempDirectory();
        WriteAddon(temp.Path, fileOnDisk: false);

        Assert.Equal(AddonHealth.Incomplete, Assert.Single(await AddonScanner.ScanAsync(temp.Path)).Health);
    }

    [Fact]
    public async Task Wrong_file_size_means_incomplete()
    {
        using var temp = new TempDirectory();
        WriteAddon(temp.Path, fileContent: "123");

        Assert.Equal(AddonHealth.Incomplete, Assert.Single(await AddonScanner.ScanAsync(temp.Path)).Health);
    }

    [Fact]
    public async Task Corrupted_flag_is_reported()
    {
        using var temp = new TempDirectory();
        WriteAddon(temp.Path, corrupted: true);

        Assert.Equal(AddonHealth.Corrupted, Assert.Single(await AddonScanner.ScanAsync(temp.Path)).Health);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("{ kaputt")]
    [InlineData("""{"meta":{"id":"AAAAAAAAAAAAAAAA","versions":[]}}""")]
    public async Task Missing_or_foreign_meta_keeps_id_from_folder(string? meta)
    {
        using var temp = new TempDirectory();
        var folder = Path.Combine(temp.Path, $"Test-Mod_{Id}");
        Directory.CreateDirectory(folder);
        if (meta is not null)
        {
            File.WriteAllText(Path.Combine(folder, "meta"), meta);
        }

        var addon = Assert.Single(await AddonScanner.ScanAsync(temp.Path));

        Assert.Equal(Id, addon.Id);
        Assert.Equal(AddonHealth.UnknownMetadata, addon.Health);
    }

    [Fact]
    public void Download_folder_maps_to_mods_root() =>
        Assert.Equal(@"D:\Mods\addons\addons", AddonPaths.ResolveModsRoot(@"D:\Mods"));

    [Theory]
    [InlineData(@"D:\Mods", @"D:\Mods")]
    [InlineData(@"D:\Mods\", @"D:\Mods")]
    [InlineData(@"D:\Mods\addons", @"D:\Mods")]
    [InlineData(@"D:\Mods\addons\addons", @"D:\Mods")]
    [InlineData(@"D:\Mods\ADDONS\addons\", @"D:\Mods")]
    [InlineData(@"D:\addons", @"D:\")]
    [InlineData("  ", null)]
    [InlineData(null, null)]
    public void Chosen_mod_folder_is_not_nested_twice(string? chosen, string? expected) =>
        Assert.Equal(expected, AddonPaths.NormalizeDownloadDir(chosen));

    [Fact]
    public void Folder_with_the_mods_maps_to_itself() =>
        Assert.Equal(@"E:\Mods\addons\addons", AddonPaths.ResolveModsRoot(@"E:\Mods\addons\addons"));

    [Fact]
    public void Without_download_folder_the_client_default_is_used() =>
        Assert.EndsWith(@"My Games\ArmaReforger\addons", AddonPaths.ResolveModsRoot(null));
}
