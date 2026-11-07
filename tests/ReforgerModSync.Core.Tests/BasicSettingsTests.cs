using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Tests;

public class BasicSettingsTests
{
    private static readonly AppSettings Current = new()
    {
        Profiles = [new ServerProfile { Id = "1", Name = "Training", ServerAddress = "203.0.113.10:2001" }],
        CleanupApprovedFolders = [@"D:\Mods"],
        Client = new ClientWindowSettings { Width = 800, Height = 450, MaxFps = 30, HideWindow = false },
    };

    private static BasicSettingsInput Valid => BasicSettingsInput.Defaults;

    [Fact]
    public void Applies_values_and_keeps_everything_else()
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { TotalMinutes = "90", IdleMinutes = "5" });

        Assert.True(result.IsValid);
        var settings = result.Settings!;
        Assert.Equal((90, 5), (settings.Timeouts.TotalMinutes, settings.Timeouts.IdleMinutes));
        Assert.Same(Current.Client, settings.Client);
        Assert.Same(Current.Profiles, settings.Profiles);
        Assert.Same(Current.CleanupApprovedFolders, settings.CleanupApprovedFolders);
        Assert.Null(settings.ReforgerExe);
    }

    [Fact]
    public void Defaults_match_a_fresh_installation()
    {
        Assert.Equal(new BasicSettingsInput("", "120", "10"), BasicSettingsInput.Defaults);
    }

    [Theory]
    [InlineData("9")]
    [InlineData("601")]
    [InlineData("")]
    [InlineData("abc")]
    [InlineData("-120")]
    [InlineData("1 20")]
    public void Rejects_invalid_total_time(string total)
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { TotalMinutes = total });

        Assert.False(result.IsValid);
        Assert.Equal("Bitte eine Zahl von 10 bis 600 eintragen.", result.Errors[BasicSettingsField.TotalMinutes]);
    }

    [Fact]
    public void Accepts_the_limits_and_surrounding_spaces()
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { TotalMinutes = " 600 ", IdleMinutes = "2" });

        Assert.True(result.IsValid);
    }

    [Fact]
    public void Reports_every_invalid_field()
    {
        var result = BasicSettingsValidator.Apply(Current, new BasicSettingsInput(@"C:\fehlt\ArmaReforgerSteam.exe", "0", "0"));

        Assert.Equal(3, result.Errors.Count);
    }

    [Fact]
    public void Idle_time_must_be_below_total_time()
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { TotalMinutes = "30", IdleMinutes = "30" });

        Assert.Equal("Muss kleiner als die Höchstdauer sein.", result.Errors[BasicSettingsField.IdleMinutes]);
    }

    [Fact]
    public void Accepts_an_existing_client_exe()
    {
        using var temp = new TempDirectory();
        var exe = Path.Combine(temp.Path, SteamLocator.ClientExeName);
        File.WriteAllText(exe, "");

        var result = BasicSettingsValidator.Apply(Current, Valid with { ReforgerExe = $"  \"{exe}\" " });

        Assert.Equal(exe, result.Settings!.ReforgerExe);
    }

    [Fact]
    public void Rejects_a_missing_or_wrong_client_exe()
    {
        using var temp = new TempDirectory();
        var other = Path.Combine(temp.Path, "ArmaReforger_BE.exe");
        File.WriteAllText(other, "");

        var missing = BasicSettingsValidator.Apply(Current, Valid with { ReforgerExe = Path.Combine(temp.Path, SteamLocator.ClientExeName) });
        var wrong = BasicSettingsValidator.Apply(Current, Valid with { ReforgerExe = other });

        Assert.Equal("Die Datei gibt es nicht.", missing.Errors[BasicSettingsField.ReforgerExe]);
        Assert.Equal("Bitte die Datei ArmaReforgerSteam.exe aus dem Spielordner wählen.", wrong.Errors[BasicSettingsField.ReforgerExe]);
    }

    [Fact]
    public void Empty_client_path_means_automatic()
    {
        var withExe = Current with { ReforgerExe = @"C:\Spiele\ArmaReforgerSteam.exe" };

        var result = BasicSettingsValidator.Apply(withExe, Valid with { ReforgerExe = "   " });

        Assert.Null(result.Settings!.ReforgerExe);
    }

    [Fact]
    public void Input_reflects_saved_settings()
    {
        var saved = new AppSettings
        {
            ReforgerExe = @"C:\Spiele\ArmaReforgerSteam.exe",
            Timeouts = new TimeoutSettings { TotalMinutes = 90, IdleMinutes = 5 },
        };

        Assert.Equal(new BasicSettingsInput(@"C:\Spiele\ArmaReforgerSteam.exe", "90", "5"), BasicSettingsInput.From(saved));
    }
}
