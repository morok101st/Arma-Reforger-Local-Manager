using ReforgerModSync.Core;

namespace ReforgerModSync.Core.Tests;

public class AppInfoTests
{
    [Theory]
    [InlineData("0.3.1+25592579abcdef", "0.3.1")]
    [InlineData("0.3.1", "0.3.1")]
    [InlineData("0.4.0-rc1+25592579", "0.4.0-rc1")]
    [InlineData("0.0.0-dev+25592579", "0.0.0-dev")]
    [InlineData("", "unbekannt")]
    [InlineData(null, "unbekannt")]
    public void FormatVersion_ShowsVersionWithoutCommit(string? informational, string expected)
    {
        Assert.Equal(expected, AppInfo.FormatVersion(informational));
    }
}
