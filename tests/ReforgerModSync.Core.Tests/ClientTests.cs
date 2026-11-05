using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Tests;

public class ClientTests
{
    private static readonly ModEntry[] Mods =
    [
        new("65AD7C139EB4C1A1", "ACE Backblast Dev", "1.5.36"),
        new("64EE818E08AFCF94", "MFD Framework"),
    ];

    private static IReadOnlyList<string> Download(string? downloadDir) =>
        ClientArgsBuilder.BuildDownload("203.0.113.10:2001", Mods, downloadDir, new ClientWindowSettings(), @"C:\logs", @"C:\profil", @"C:\app\helper");

    private static IReadOnlyList<string> ListRequest() =>
        ClientArgsBuilder.BuildListRequest("203.0.113.10:2001", @"D:\Mods", new ClientWindowSettings(), @"C:\logs", @"C:\profil", @"C:\app\helper");

    [Theory]
    [InlineData("11:46:47.145 BACKEND      : Addon Download started 65AD7C139EB4C1A1 - ACE Backblast Dev", ClientLogEvent.Activity)]
    [InlineData("11:46:48.347 BACKEND      : Downloading 65AD7C139EB4C1A1 version 1.5.36", ClientLogEvent.Activity)]
    [InlineData("11:47:52.528 BACKEND      : Updating 64EE818E08AFCF94 from 0.5.6 to 0.5.8", ClientLogEvent.Activity)]
    [InlineData("11:46:47.145 SCRIPT       : [RMS] DOWNLOAD_STARTED 65AD7C139EB4C1A1 1.5.36", ClientLogEvent.Activity)]
    [InlineData("11:46:50.746 SCRIPT       : [RMS] DONE failed=0", ClientLogEvent.HelperDone)]
    [InlineData("11:46:50.746 SCRIPT       : [RMS] MOD_FAILED 65AD7C139EB4C1A1 not found", ClientLogEvent.ModFailed)]
    [InlineData(@"09:49:27.425 BACKEND   (E): Cannot move a file from C:\a to E:\b", ClientLogEvent.BackendError)]
    [InlineData("09:54:40.597 BACKEND      : Addon Remove 6AAD0E4883FE0B85 - ROG CQB Training Simulation", ClientLogEvent.None)]
    public void Real_log_lines_are_classified(string line, ClientLogEvent expected) =>
        Assert.Equal(expected, ClientLogParser.Classify(line));

    [Fact]
    public void Failed_mod_line_gives_id_and_reason() =>
        Assert.Equal(
            ("65AD7C139EB4C1A1", "version 1.5.36 not available"),
            ClientLogParser.ParseModFailed("11:46:50 SCRIPT : [RMS] MOD_FAILED 65AD7C139EB4C1A1 version 1.5.36 not available"));

    [Fact]
    public void Server_mod_line_gives_id_version_and_name_with_spaces() =>
        Assert.Equal(
            new ModEntry("1337C0DE5DABBEEF", "RHS - Content Pack 01", "0.16.5236"),
            ClientLogParser.ParseServerMod("12:35:56 SCRIPT       : [RMS] SERVER_MOD 1337C0DE5DABBEEF 0.16.5236 RHS - Content Pack 01"));

    [Fact]
    public void Server_mod_line_without_version_means_any_version() =>
        Assert.Equal(
            new ModEntry("1337C0DE5DABBEEF", "Pack"),
            ClientLogParser.ParseServerMod("12:35:56 SCRIPT : [RMS] SERVER_MOD 1337C0DE5DABBEEF  Pack"));

    [Theory]
    [InlineData("[RMS] LIST_DONE mods=76", ServerListEnd.Done)]
    [InlineData("[RMS] ROOM_NOT_FOUND", ServerListEnd.NotFound)]
    [InlineData("[RMS] ROOM_PASSWORD_REQUIRED", ServerListEnd.PasswordRequired)]
    [InlineData("[RMS] ROOM_PASSWORD_REJECTED http=403", ServerListEnd.PasswordRejected)]
    [InlineData("[RMS] ROOM_SEARCH_FAILED http=500", ServerListEnd.Failed)]
    [InlineData("[RMS] ROOM_LIST_FAILED http=500", ServerListEnd.Failed)]
    [InlineData("[RMS] ROOM_FOUND rooms=1", null)]
    [InlineData("[RMS] SERVER_MOD 1337C0DE5DABBEEF 1.0 ROOM_NOT_FOUND Pack", null)]
    public void Server_list_end_is_recognized(string line, ServerListEnd? expected) =>
        Assert.Equal(expected, ClientLogParser.ParseServerListEnd(line));

    [Fact]
    public void Tail_returns_only_new_complete_lines()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "console.log");
        var tail = new LogTail(path);
        Assert.Empty(tail.ReadNewLines());

        File.WriteAllText(path, "eins\r\nzw");
        Assert.Equal(["eins"], tail.ReadNewLines());

        File.AppendAllText(path, "ei\ndrei\n");
        Assert.Equal(["zwei", "drei"], tail.ReadNewLines());
        Assert.Empty(tail.ReadNewLines());
    }

    [Fact]
    public void Tail_can_read_while_the_client_keeps_the_file_open()
    {
        using var temp = new TempDirectory();
        var path = Path.Combine(temp.Path, "console.log");
        using var writer = new StreamWriter(new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.Read)) { AutoFlush = true };
        writer.Write("zeile\n");

        Assert.Equal(["zeile"], new LogTail(path).ReadNewLines());
    }

    [Fact]
    public void Download_arguments_load_the_helper_and_keep_the_players_profile_untouched()
    {
        var args = Download(@"D:\Meine Mods");

        AssertValue(args, "-addonsDir", @"D:\Meine Mods,C:\app\helper");
        AssertValue(args, "-addonDownloadDir", @"D:\Meine Mods");
        AssertValue(args, "-addons", ClientArgsBuilder.HelperGuid);
        AssertValue(args, "-profile", @"C:\profil");
        AssertValue(args, "-logsDir", @"C:\logs");
        AssertValue(args, "-rmsServer", "203.0.113.10:2001");
        AssertValue(args, "-rmsMods", "65AD7C139EB4C1A1:1.5.36;64EE818E08AFCF94");
        Assert.Contains("-noSound", args);
    }

    [Fact]
    public void List_request_asks_for_the_server_and_contains_no_password()
    {
        var args = ListRequest();

        AssertValue(args, "-rmsServer", "203.0.113.10:2001");
        Assert.DoesNotContain("-rmsMods", args);
        Assert.DoesNotContain(args, arg => arg.Contains("password", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void The_client_never_connects_to_the_game_server()
    {
        Assert.DoesNotContain("-client", Download(@"D:\Mods"));
        Assert.DoesNotContain("-client", ListRequest());
    }

    [Fact]
    public void Without_folder_the_clients_default_is_set_explicitly_because_of_the_own_profile()
    {
        var args = Download(null);

        Assert.EndsWith(@"My Games\ArmaReforger", args[args.ToList().IndexOf("-addonDownloadDir") + 1]);
        Assert.EndsWith(@"My Games\ArmaReforger,C:\app\helper", args[args.ToList().IndexOf("-addonsDir") + 1]);
    }

    [Fact]
    public void Join_uses_the_helper_and_the_players_own_profile()
    {
        var args = ClientArgsBuilder.BuildJoin("203.0.113.10:2001", @"D:\Meine Mods", @"C:\app\helper");

        AssertValue(args, "-addonsDir", @"D:\Meine Mods,C:\app\helper");
        AssertValue(args, "-addonDownloadDir", @"D:\Meine Mods");
        AssertValue(args, "-addons", ClientArgsBuilder.HelperGuid);
        AssertValue(args, "-rmsJoin", "203.0.113.10:2001");
        Assert.DoesNotContain("-profile", args);
        Assert.DoesNotContain("-client", args);
    }

    [Fact]
    public void Join_without_folder_keeps_the_games_own_mod_folder()
    {
        var args = ClientArgsBuilder.BuildJoin("203.0.113.10:2001", null, @"C:\app\helper");

        AssertValue(args, "-addonsDir", @"C:\app\helper");
        Assert.DoesNotContain("-addonDownloadDir", args);
    }

    [Fact]
    public void Command_line_quotes_values_with_spaces() =>
        Assert.Equal(
            """-exe ArmaReforgerSteam.exe -addonsDir "D:\Meine Mods,C:\app helper\x" """.TrimEnd(),
            ClientArgsBuilder.ToCommandLine(["-exe", "ArmaReforgerSteam.exe", "-addonsDir", @"D:\Meine Mods,C:\app helper\x"]));

    [Fact]
    public void Steam_launch_options_are_quoted_for_paths_with_spaces() =>
        Assert.Equal(
            """-addonsDir "D:\Meine Mods" -addonDownloadDir "D:\Meine Mods" """.TrimEnd(),
            ClientArgsBuilder.SteamLaunchOptions(@"D:\Meine Mods"));

    [Fact]
    public void Default_folder_needs_no_steam_launch_options() =>
        Assert.Equal("", ClientArgsBuilder.SteamLaunchOptions(null));

    [Theory]
    [InlineData("203.0.113.10:2001", true)]
    [InlineData(" 203.0.113.10:2001 ", true)]
    [InlineData("server.example.org:2001", false)]
    [InlineData("203.0.113.10", false)]
    [InlineData("", false)]
    [InlineData(null, false)]
    [InlineData("999.1.1.1:2001", false)]
    [InlineData("203.0.113.10:70000", false)]
    [InlineData("203.0.113.10:0", false)]
    [InlineData("203.0.113.10:", false)]
    [InlineData(":2001", false)]
    public void Server_address_needs_ip_and_port(string? address, bool valid) =>
        Assert.Equal(valid, ClientArgsBuilder.IsValidServerAddress(address));

    [Theory]
    [InlineData("0.0.0.0", true)]
    [InlineData("255.255.255.255", true)]
    [InlineData(" 203.0.113.10 ", true)]
    [InlineData("256.1.1.1", false)]
    [InlineData("1.2.3", false)]
    [InlineData("1.2.3.4.5", false)]
    [InlineData("01.2.3.4", false)]
    [InlineData("server.example.org", false)]
    [InlineData("", false)]
    [InlineData(null, false)]
    public void Ip_must_be_a_real_ipv4_address(string? ip, bool valid) =>
        Assert.Equal(valid, ClientArgsBuilder.IsValidIp(ip));

    [Theory]
    [InlineData("1", true)]
    [InlineData("2001", true)]
    [InlineData("65535", true)]
    [InlineData("0", false)]
    [InlineData("65536", false)]
    [InlineData("abc", false)]
    [InlineData("", false)]
    [InlineData(null, false)]
    public void Port_must_be_in_range(string? port, bool valid) =>
        Assert.Equal(valid, ClientArgsBuilder.IsValidPort(port));

    [Theory]
    [InlineData("203.0.113.10:2001", "203.0.113.10", "2001")]
    [InlineData(" 203.0.113.10:2001 ", "203.0.113.10", "2001")]
    [InlineData("203.0.113.10", "203.0.113.10", "")]
    [InlineData("203.0.113.10:", "203.0.113.10", "")]
    [InlineData("", "", "")]
    [InlineData(null, "", "")]
    public void Splits_address_for_the_input_fields(string? address, string ip, string port) =>
        Assert.Equal((ip, port), ClientArgsBuilder.SplitServerAddress(address));

    [Theory]
    [InlineData("203.0.113.10", "2001", "203.0.113.10:2001")]
    [InlineData(" 203.0.113.10 ", " 2001 ", "203.0.113.10:2001")]
    [InlineData("203.0.113.10", "02001", "203.0.113.10:2001")]
    [InlineData("203.0.113.10", "", "203.0.113.10:")]
    [InlineData("", "", null)]
    [InlineData(null, null, null)]
    public void Combines_fields_to_the_stored_address(string? ip, string? port, string? expected) =>
        Assert.Equal(expected, ClientArgsBuilder.CombineServerAddress(ip, port));

    [Theory]
    [InlineData("203.0.113.10:2001")]
    [InlineData("1.1.1.1:65535")]
    public void Split_and_combine_keep_a_saved_address_unchanged(string address)
    {
        // Das Passwort hängt an diesem Wert. Ändert er sich beim Laden, ginge die Zuordnung verloren.
        var (ip, port) = ClientArgsBuilder.SplitServerAddress(address);
        Assert.Equal(address, ClientArgsBuilder.CombineServerAddress(ip, port));
    }

    [Theory]
    [InlineData("2001", "2001")]
    [InlineData("20x01 ", "2001")]
    [InlineData("abc", "")]
    [InlineData("123456789", "12345")]
    [InlineData("", "")]
    [InlineData(null, "")]
    public void Port_field_keeps_only_digits(string? input, string expected) =>
        Assert.Equal(expected, ClientArgsBuilder.CleanPortInput(input));

    private static void AssertValue(IReadOnlyList<string> args, string name, string value) =>
        Assert.Equal(value, args[args.ToList().IndexOf(name) + 1]);
}
