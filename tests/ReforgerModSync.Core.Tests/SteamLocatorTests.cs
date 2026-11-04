using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Tests;

public class SteamLocatorTests
{
    [Fact]
    public void Finds_the_client_in_a_second_library()
    {
        using var temp = new TempDirectory();
        var steam = Path.Combine(temp.Path, "Steam");
        var library = Path.Combine(temp.Path, "Other Library");
        Directory.CreateDirectory(Path.Combine(steam, "steamapps"));
        File.WriteAllText(
            Path.Combine(steam, "steamapps", "libraryfolders.vdf"),
            $$"""
            "libraryfolders"
            {
                "0" { "path"  "{{steam.Replace(@"\", @"\\")}}" }
                "1" { "path"  "{{library.Replace(@"\", @"\\")}}" }
            }
            """);

        var install = Path.Combine(library, "steamapps", "common", "Arma Reforger");
        Directory.CreateDirectory(install);
        File.WriteAllText(Path.Combine(library, "steamapps", "appmanifest_1874880.acf"), """
            "AppState" { "appid" "1874880" "installdir" "Arma Reforger" }
            """);
        var exe = Path.Combine(install, "ArmaReforgerSteam.exe");
        File.WriteAllText(exe, "");

        Assert.Equal(exe, SteamLocator.FindClientExe(steam));
    }

    [Fact]
    public void Returns_null_without_client_or_library_file()
    {
        using var temp = new TempDirectory();

        Assert.Null(SteamLocator.FindClientExe(temp.Path));
    }
}
