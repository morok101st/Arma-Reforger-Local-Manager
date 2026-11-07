using System.Runtime.InteropServices;
using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Tests;

public class CommandLineTests
{
    [DllImport("shell32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern IntPtr CommandLineToArgvW(string commandLine, out int count);

    [DllImport("kernel32.dll")]
    private static extern IntPtr LocalFree(IntPtr memory);

    /// <summary>Zerlegt eine Befehlszeile so, wie Windows-Programme es tun.</summary>
    private static string[] Split(string commandLine)
    {
        // Das erste Element gilt als Programmname und hat eigene Regeln, deshalb ein fester Platzhalter davor.
        var argv = CommandLineToArgvW("programm.exe " + commandLine, out var count);
        try
        {
            return Enumerable.Range(1, count - 1).Select(i => Marshal.PtrToStringUni(Marshal.ReadIntPtr(argv, i * IntPtr.Size))!).ToArray();
        }
        finally
        {
            LocalFree(argv);
        }
    }

    [Theory]
    [InlineData(@"D:\Meine Mods,C:\app helper\x")]
    [InlineData(@"D:\Mods\")]
    [InlineData(@"D:\Meine Mods\")]
    [InlineData(@"C:\")]
    [InlineData("")]
    [InlineData("mit \"Anführungszeichen\"")]
    [InlineData(@"Backslash vor \""Zeichen")]
    [InlineData("Tab\tdarin")]
    [InlineData("1A2B3C4D5E6F7001:1.4.0;1A2B3C4D5E6F7002")]
    public void Every_value_survives_the_round_trip(string value)
    {
        string[] arguments = ["-exe", "ArmaReforgerSteam.exe", "-addonsDir", value, "-rmsJoin", "203.0.113.10:2001"];

        Assert.Equal(arguments, Split(ClientArgsBuilder.ToCommandLine(arguments)));
    }

    [Fact]
    public void Simple_values_stay_unquoted() =>
        Assert.Equal("-addons 7A3C1E5D9B2F4860 -rmsJoin 203.0.113.10:2001", ClientArgsBuilder.ToCommandLine(["-addons", "7A3C1E5D9B2F4860", "-rmsJoin", "203.0.113.10:2001"]));

    [Theory]
    [InlineData(@"D:\")]
    [InlineData(@"D:\Meine Mods")]
    public void Steam_launch_options_split_back_into_the_folder(string folder) =>
        Assert.Equal(["-addonsDir", folder, "-addonDownloadDir", folder], Split(ClientArgsBuilder.SteamLaunchOptions(folder)));

    [Fact]
    public void A_trailing_backslash_does_not_swallow_the_closing_quote() =>
        Assert.Equal(@"""D:\Meine Mods\\""", ClientArgsBuilder.ToCommandLine([@"D:\Meine Mods\"]));
}
