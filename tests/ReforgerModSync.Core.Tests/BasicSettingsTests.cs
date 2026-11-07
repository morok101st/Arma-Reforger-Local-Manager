using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Tests;

public class BasicSettingsTests
{
    private static readonly AppSettings Current = new()
    {
        Profiles = [new ServerProfile { Id = "1", Name = "Training", ServerAddress = "203.0.113.10:2001" }],
        CleanupApprovedFolders = [@"D:\Mods"],
        Client = new ClientWindowSettings { HideWindow = false },
    };

    private static BasicSettingsInput Valid => BasicSettingsInput.Defaults;

    [Fact]
    public void Applies_values_and_keeps_everything_else()
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { Width = "800", Height = "450", MaxFps = "30", TotalMinutes = "90", IdleMinutes = "5" });

        Assert.True(result.IsValid);
        var settings = result.Settings!;
        Assert.Equal((800, 450, 30), (settings.Client.Width, settings.Client.Height, settings.Client.MaxFps));
        Assert.Equal((90, 5), (settings.Timeouts.TotalMinutes, settings.Timeouts.IdleMinutes));
        Assert.False(settings.Client.HideWindow);
        Assert.Same(Current.Profiles, settings.Profiles);
        Assert.Same(Current.CleanupApprovedFolders, settings.CleanupApprovedFolders);
        Assert.Null(settings.ReforgerExe);
    }

    [Fact]
    public void Defaults_match_a_fresh_installation()
    {
        Assert.Equal(new BasicSettingsInput("", "640", "360", "15", "120", "10"), BasicSettingsInput.Defaults);
    }

    [Theory]
    [InlineData("319")]
    [InlineData("3841")]
    [InlineData("")]
    [InlineData("abc")]
    [InlineData("-640")]
    [InlineData("6 40")]
    public void Rejects_invalid_width(string width)
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { Width = width });

        Assert.False(result.IsValid);
        Assert.Equal("Bitte eine Zahl von 320 bis 3840 eintragen.", result.Errors[BasicSettingsField.Width]);
    }

    [Fact]
    public void Accepts_the_limits_and_surrounding_spaces()
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { Width = " 320 ", Height = "2160", MaxFps = "60", TotalMinutes = "600", IdleMinutes = "2" });

        Assert.True(result.IsValid);
    }

    [Fact]
    public void Reports_every_invalid_field()
    {
        var result = BasicSettingsValidator.Apply(Current, new BasicSettingsInput("", "0", "0", "0", "0", "0"));

        Assert.Equal(5, result.Errors.Count);
    }

    [Fact]
    public void Idle_time_must_be_below_total_time()
    {
        var result = BasicSettingsValidator.Apply(Current, Valid with { TotalMinutes = "30", IdleMinutes = "30" });

        Assert.Equal("Muss kleiner als die Gesamtzeit sein.", result.Errors[BasicSettingsField.IdleMinutes]);
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
            Client = new ClientWindowSettings { Width = 800, Height = 450, MaxFps = 30 },
            Timeouts = new TimeoutSettings { TotalMinutes = 90, IdleMinutes = 5 },
        };

        Assert.Equal(new BasicSettingsInput(@"C:\Spiele\ArmaReforgerSteam.exe", "800", "450", "30", "90", "5"), BasicSettingsInput.From(saved));
    }
}
