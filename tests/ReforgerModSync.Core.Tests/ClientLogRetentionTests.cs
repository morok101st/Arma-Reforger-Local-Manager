using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Tests;

public class ClientLogRetentionTests
{
    private static string Run(TempDirectory temp, int minute)
    {
        var folder = Path.Combine(temp.Path, $"20261107-12{minute:00}00-000");
        Directory.CreateDirectory(folder);
        File.WriteAllText(Path.Combine(folder, "console.log"), "log");
        return folder;
    }

    [Fact]
    public void Keeps_only_the_newest_runs()
    {
        using var temp = new TempDirectory();
        var runs = Enumerable.Range(0, 12).Select(minute => Run(temp, minute)).ToList();

        var deleted = ClientLogRetention.Prune(temp.Path, 10);

        Assert.Equal(2, deleted);
        Assert.False(Directory.Exists(runs[0]));
        Assert.False(Directory.Exists(runs[1]));
        Assert.All(runs.Skip(2), run => Assert.True(Directory.Exists(run)));
    }

    [Fact]
    public void Also_removes_run_folders_without_milliseconds()
    {
        using var temp = new TempDirectory();
        var old = Directory.CreateDirectory(Path.Combine(temp.Path, "20261001-120000")).FullName;
        var newer = Run(temp, 1);

        var deleted = ClientLogRetention.Prune(temp.Path, 1);

        Assert.Equal(1, deleted);
        Assert.False(Directory.Exists(old));
        Assert.True(Directory.Exists(newer));
    }

    [Fact]
    public void Sorts_by_name_not_by_write_time()
    {
        using var temp = new TempDirectory();
        var older = Run(temp, 1);
        var newer = Run(temp, 2);
        Directory.SetLastWriteTime(older, DateTime.Now);
        Directory.SetLastWriteTime(newer, DateTime.Now.AddDays(-1));

        ClientLogRetention.Prune(temp.Path, 1);

        Assert.False(Directory.Exists(older));
        Assert.True(Directory.Exists(newer));
    }

    [Fact]
    public void Leaves_other_folders_and_files_alone()
    {
        using var temp = new TempDirectory();
        var foreign = Directory.CreateDirectory(Path.Combine(temp.Path, "eigene Notizen")).FullName;
        var file = Path.Combine(temp.Path, "20261107-120000-000.txt");
        File.WriteAllText(file, "x");
        Run(temp, 1);
        Run(temp, 2);

        ClientLogRetention.Prune(temp.Path, 0);

        Assert.True(Directory.Exists(foreign));
        Assert.True(File.Exists(file));
        Assert.Empty(Directory.GetDirectories(temp.Path, "2026*"));
    }

    [Fact]
    public void Skips_a_locked_folder()
    {
        using var temp = new TempDirectory();
        var locked = Run(temp, 1);
        var free = Run(temp, 2);
        Run(temp, 3);

        int deleted;
        using (new FileStream(Path.Combine(locked, "console.log"), FileMode.Open, FileAccess.Read, FileShare.None))
        {
            deleted = ClientLogRetention.Prune(temp.Path, 1);
        }

        Assert.Equal(1, deleted);
        Assert.True(Directory.Exists(locked));
        Assert.False(Directory.Exists(free));
    }

    [Fact]
    public void Does_nothing_when_the_folder_is_missing()
    {
        using var temp = new TempDirectory();

        Assert.Equal(0, ClientLogRetention.Prune(Path.Combine(temp.Path, "fehlt")));
    }

    [Fact]
    public void CreateRunFolder_keeps_exactly_the_newest_runs()
    {
        using var temp = new TempDirectory();
        Enumerable.Range(0, 12).ToList().ForEach(minute => Run(temp, minute));

        var folder = ClientLogRetention.CreateRunFolder(temp.Path);

        Assert.True(Directory.Exists(folder));
        Assert.Equal(ClientLogRetention.KeepRuns, Directory.GetDirectories(temp.Path).Length);
        Assert.Contains(folder, Directory.GetDirectories(temp.Path));
    }

    [Fact]
    public void Run_folder_names_match_the_pattern()
    {
        using var temp = new TempDirectory();
        Directory.CreateDirectory(Path.Combine(temp.Path, DateTime.Now.ToString(ClientLogRetention.RunFolderFormat)));

        Assert.Equal(1, ClientLogRetention.Prune(temp.Path, 0));
    }
}
