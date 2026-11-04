using ReforgerModSync.Core.Addons;

namespace ReforgerModSync.Core.Tests;

public class AddonComparerTests
{
    private const string Id = "660E395AD366D1F7";

    private static ModState Compare(string? requiredVersion, InstalledAddon? installed) =>
        Assert.Single(AddonComparer.Compare(
            [new ModEntry(Id, "Mod", requiredVersion)],
            installed is null ? [] : [installed])).State;

    private static InstalledAddon Addon(string version = "1.0.0", AddonHealth health = AddonHealth.Ok) =>
        new(Id, "Mod", version, "ordner", health);

    [Fact]
    public void Not_installed_is_missing() => Assert.Equal(ModState.Missing, Compare("1.0.0", null));

    [Fact]
    public void Same_version_is_current() => Assert.Equal(ModState.Current, Compare("1.0.0", Addon()));

    [Fact]
    public void Older_local_version_is_outdated() => Assert.Equal(ModState.Outdated, Compare("1.0.1", Addon()));

    [Fact]
    public void Newer_local_version_is_not_called_outdated() =>
        Assert.Equal(ModState.Newer, Compare("0.7.33", Addon("0.7.34")));

    [Fact]
    public void Version_parts_are_compared_as_numbers_not_as_text() =>
        Assert.Equal(ModState.Outdated, Compare("0.10.0", Addon("0.9.71")));

    [Fact]
    public void Versions_of_different_length_are_compared() =>
        Assert.Equal(ModState.Newer, Compare("1.0", Addon("1.0.1")));

    [Fact]
    public void Same_version_written_differently_is_current() =>
        Assert.Equal(ModState.Current, Compare("1.0", Addon("1.0.0")));

    [Fact]
    public void Version_that_cannot_be_compared_is_just_different() =>
        Assert.Equal(ModState.Different, Compare("1.0.0-beta", Addon("1.0.0")));

    [Fact]
    public void Modset_without_version_accepts_any_healthy_version() =>
        Assert.Equal(ModState.Current, Compare(null, Addon()));

    [Theory]
    [InlineData(AddonHealth.Incomplete)]
    [InlineData(AddonHealth.Corrupted)]
    [InlineData(AddonHealth.UnknownMetadata)]
    public void Unhealthy_addon_is_damaged_even_with_matching_version(AddonHealth health) =>
        Assert.Equal(ModState.Damaged, Compare("1.0.0", Addon(health: health)));

    [Fact]
    public void Ids_match_regardless_of_case()
    {
        var result = AddonComparer.Compare([new ModEntry(Id.ToLowerInvariant(), "Mod")], [Addon()]);

        Assert.Equal(ModState.Current, Assert.Single(result).State);
    }
}
