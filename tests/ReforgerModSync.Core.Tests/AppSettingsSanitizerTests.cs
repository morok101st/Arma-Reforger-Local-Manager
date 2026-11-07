using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Tests;

public class AppSettingsSanitizerTests
{
    [Fact]
    public void Valid_settings_stay_unchanged()
    {
        var settings = new AppSettings
        {
            Profiles = [new ServerProfile { Id = "a", Name = "Training", ServerAddress = "203.0.113.10:2001" }],
            ReforgerExe = @"C:\Spiele\ArmaReforgerSteam.exe",
            Client = new ClientWindowSettings { Width = 800, Height = 450, MaxFps = 30, HideWindow = false },
            Timeouts = new TimeoutSettings { TotalMinutes = 90, IdleMinutes = 5 },
            CleanupApprovedFolders = [@"D:\MODS"],
        };

        var clean = AppSettingsSanitizer.Sanitize(settings);

        Assert.Equal(settings.Profiles, clean.Profiles);
        Assert.Equal(settings.CleanupApprovedFolders, clean.CleanupApprovedFolders);
        Assert.Equal(settings.Client, clean.Client);
        Assert.Equal(settings.Timeouts, clean.Timeouts);
        Assert.Equal(settings.ReforgerExe, clean.ReforgerExe);
    }

    [Theory]
    [InlineData(0, 10)]
    [InlineData(-5, 10)]
    [InlineData(120, 0)]
    [InlineData(100000, 10)]
    [InlineData(30, 30)]
    [InlineData(20, 50)]
    public void Invalid_timeouts_fall_back_to_safe_values(int total, int idle)
    {
        var clean = AppSettingsSanitizer.Sanitize(new AppSettings { Timeouts = new TimeoutSettings { TotalMinutes = total, IdleMinutes = idle } });

        Assert.InRange(clean.Timeouts.TotalMinutes, BasicSettingsValidator.MinTotalMinutes, BasicSettingsValidator.MaxTotalMinutes);
        Assert.InRange(clean.Timeouts.IdleMinutes, BasicSettingsValidator.MinIdleMinutes, BasicSettingsValidator.MaxIdleMinutes);
        Assert.True(clean.Timeouts.IdleMinutes < clean.Timeouts.TotalMinutes);
    }

    [Fact]
    public void Invalid_window_values_fall_back_to_the_defaults()
    {
        var clean = AppSettingsSanitizer.Sanitize(new AppSettings { Client = new ClientWindowSettings { Width = 0, Height = 99999, MaxFps = -1, HideWindow = false } });

        Assert.Equal(new ClientWindowSettings { HideWindow = false }, clean.Client);
    }

    [Fact]
    public void Missing_sections_become_defaults()
    {
        var broken = new AppSettings { Profiles = null!, Client = null!, Timeouts = null!, CleanupApprovedFolders = null! };

        var clean = AppSettingsSanitizer.Sanitize(broken);

        Assert.Empty(clean.Profiles);
        Assert.Empty(clean.CleanupApprovedFolders);
        Assert.Equal(new ClientWindowSettings(), clean.Client);
        Assert.Equal(new TimeoutSettings(), clean.Timeouts);
    }

    [Fact]
    public void Profiles_without_id_are_dropped_and_blank_exe_means_automatic()
    {
        var clean = AppSettingsSanitizer.Sanitize(new AppSettings
        {
            Profiles = [new ServerProfile { Id = " ", Name = "Kaputt" }, new ServerProfile { Id = "a", Name = "Gut" }],
            ReforgerExe = "   ",
        });

        Assert.Equal("Gut", Assert.Single(clean.Profiles).Name);
        Assert.Null(clean.ReforgerExe);
    }

    [Fact]
    public async Task Store_sanitizes_a_hand_edited_file()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "settings.json");
        File.WriteAllText(path, """
            { "profiles": null, "timeouts": { "totalMinutes": 0, "idleMinutes": -3 }, "client": null }
            """);

        var settings = await new SettingsStore(path).LoadAsync();

        Assert.Empty(settings.Profiles);
        Assert.Equal(new TimeoutSettings(), settings.Timeouts);
        Assert.Equal(new ClientWindowSettings(), settings.Client);
    }
}
