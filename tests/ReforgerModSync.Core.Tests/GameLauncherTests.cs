using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Tests;

public class GameLauncherTests
{
    [Fact]
    public void WaitUntilRemoved_FileMissing_ReturnsImmediately()
    {
        using var temp = new TempDirectory();

        Assert.True(GameLauncher.WaitUntilRemoved(Path.Combine(temp.Path, "fehlt.txt"), TimeSpan.Zero));
    }

    [Fact]
    public void WaitUntilRemoved_FileStays_ReturnsFalseAfterTimeout()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "bleibt.txt");
        File.WriteAllText(path, "x");

        Assert.False(GameLauncher.WaitUntilRemoved(path, TimeSpan.FromMilliseconds(150)));
        Assert.True(File.Exists(path));
    }

    [Fact]
    public async Task WaitUntilRemoved_FileDeletedWhileWaiting_ReturnsTrueEarly()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "wird-geloescht.txt");
        File.WriteAllText(path, "x");
        var waiting = Task.Run(() => GameLauncher.WaitUntilRemoved(path, TimeSpan.FromSeconds(30)));

        File.Delete(path);

        Assert.True(await waiting.WaitAsync(TimeSpan.FromSeconds(10)));
    }
}
