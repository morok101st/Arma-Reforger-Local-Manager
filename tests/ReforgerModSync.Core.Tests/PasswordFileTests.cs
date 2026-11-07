using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Tests;

public class PasswordFileTests
{
    [Fact]
    public void Writes_the_password_into_the_profile_folder()
    {
        using var temp = new TempDirectory();
        var path = PasswordFile.ForClientProfile(temp.Path);

        PasswordFile.Prepare(path, "geheim");

        Assert.Equal(Path.Combine(temp.Path, "profile", "rms_server_password.txt"), path);
        Assert.Equal("geheim", File.ReadAllText(path));
    }

    [Fact]
    public void Without_password_an_old_file_of_another_server_is_removed()
    {
        using var temp = new TempDirectory();
        var path = PasswordFile.ForClientProfile(temp.Path);
        PasswordFile.Prepare(path, "passwort-von-server-a");

        PasswordFile.Prepare(path, null);

        Assert.False(File.Exists(path));
    }

    [Fact]
    public void A_new_password_replaces_the_old_one()
    {
        using var temp = new TempDirectory();
        var path = PasswordFile.ForClientProfile(temp.Path);
        PasswordFile.Prepare(path, "ein-sehr-langes-altes-passwort");

        PasswordFile.Prepare(path, "neu");

        Assert.Equal("neu", File.ReadAllText(path));
    }

    [Fact]
    public void Empty_password_writes_no_file_and_needs_no_folder()
    {
        using var temp = new TempDirectory();
        var path = PasswordFile.ForClientProfile(Path.Combine(temp.Path, "fehlt"));

        PasswordFile.Prepare(path, "");

        Assert.False(File.Exists(path));
        Assert.False(Directory.Exists(Path.Combine(temp.Path, "fehlt")));
    }

    [Fact]
    public void Leftovers_of_a_crashed_run_are_removed_at_startup()
    {
        using var temp = new TempDirectory();
        var paths = new AppPaths(temp.Path);
        var leftover = PasswordFile.ForClientProfile(paths.ClientProfile);
        PasswordFile.Prepare(leftover, "geheim");

        PasswordFile.CleanUpLeftovers(paths);

        Assert.False(File.Exists(leftover));
    }

    [Fact]
    public void Leftovers_in_join_log_folders_are_removed_but_logs_stay()
    {
        using var temp = new TempDirectory();
        var paths = new AppPaths(temp.Path);
        var run = ClientLogRetention.CreateRunFolder(paths.ClientLogs);
        var console = Path.Combine(run, "console.log");
        File.WriteAllText(console, "log");
        PasswordFile.Prepare(PasswordFile.ForLogsFolder(run), "geheim");

        PasswordFile.CleanUpLeftovers(paths);

        Assert.False(File.Exists(PasswordFile.ForLogsFolder(run)));
        Assert.True(File.Exists(console));
    }

    [Fact]
    public void TryDelete_reports_a_locked_file()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, PasswordFile.FileName);
        File.WriteAllText(path, "geheim");

        using (new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.None))
        {
            Assert.False(PasswordFile.TryDelete(path));
        }

        Assert.True(PasswordFile.TryDelete(path));
        Assert.True(PasswordFile.TryDelete(path));
    }
}
