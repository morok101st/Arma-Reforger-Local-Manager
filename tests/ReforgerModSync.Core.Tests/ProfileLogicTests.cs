using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Credentials;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Sync;

namespace ReforgerModSync.Core.Tests;

public class ProfileLogicTests
{
    private sealed class MemoryStore : ICredentialStore
    {
        public Dictionary<(string Address, string Id), string> Entries { get; } = [];

        public string? ReadPassword(string serverAddress, string profileId) => Entries.GetValueOrDefault((serverAddress, profileId));

        public void SavePassword(string serverAddress, string profileId, string password) => Entries[(serverAddress, profileId)] = password;

        public bool DeletePassword(string serverAddress, string profileId) => Entries.Remove((serverAddress, profileId));
    }

    private const string Old = "203.0.113.10:2001";
    private const string New = "203.0.113.20:2002";

    [Theory]
    [InlineData("", "203.0.113.10", "2001", "Bitte einen Namen für diesen Server eintragen.")]
    [InlineData("Training", "server.example", "2001", "Bitte die IP-Adresse des Servers eintragen, zum Beispiel 203.0.113.10.")]
    [InlineData("Training", "203.0.113.10", "70000", "Bitte den Port des Servers eintragen, eine Zahl von 1 bis 65535, zum Beispiel 2001.")]
    [InlineData("Training", "203.0.113.10", "2001", null)]
    public void Profile_input_is_checked_in_order(string name, string ip, string port, string? expected) =>
        Assert.Equal(expected, ProfileInput.Validate(name, ip, port));

    [Theory]
    [InlineData("", true, Old, New, true)]
    [InlineData("neu", true, Old, New, false)]
    [InlineData("", false, Old, New, false)]
    [InlineData("", true, Old, Old, false)]
    [InlineData("", true, null, New, false)]
    public void Transfer_decision_only_when_a_saved_password_would_move(string entered, bool hasSaved, string? saved, string? next, bool expected) =>
        Assert.Equal(expected, ProfilePasswords.NeedsTransferDecision(hasSaved, saved, next, entered));

    [Fact]
    public void Moving_with_consent_keeps_the_password_under_the_new_address_only()
    {
        var store = new MemoryStore();
        store.SavePassword(Old, "a", "geheim");

        new ProfilePasswords(store).MoveToNewAddress("a", Old, New, keep: true);

        Assert.Equal("geheim", store.ReadPassword(New, "a"));
        Assert.Null(store.ReadPassword(Old, "a"));
    }

    [Fact]
    public void Moving_without_consent_leaves_no_password_behind()
    {
        var store = new MemoryStore();
        store.SavePassword(Old, "a", "geheim");

        new ProfilePasswords(store).MoveToNewAddress("a", Old, New, keep: false);

        Assert.Empty(store.Entries);
    }

    [Fact]
    public void An_empty_entry_keeps_the_saved_password()
    {
        var store = new MemoryStore();
        var passwords = new ProfilePasswords(store);
        store.SavePassword(Old, "a", "geheim");

        passwords.StoreEntered(Old, "a", "");

        Assert.Equal("geheim", store.ReadPassword(Old, "a"));
        Assert.True(passwords.HasPassword(Old, "a"));
        Assert.False(passwords.HasPassword(Old, "b"));
        Assert.False(passwords.HasPassword(null, "a"));
    }

    private static ModComparison Mod(string name, ModState state) =>
        new(new ModEntry(name.PadRight(16, '0')[..16], name, "1.0.0"), state, null);

    [Fact]
    public void Summary_counts_every_kind_of_open_mod()
    {
        var summary = ModSummary.From(
        [
            Mod("A", ModState.Current), Mod("B", ModState.Current), Mod("C", ModState.Missing),
            Mod("D", ModState.Outdated), Mod("E", ModState.Damaged),
        ]);

        Assert.Equal((5, 2, 3), (summary.Total, summary.Current, summary.Open));
        Assert.Equal("3 von 5 offen", summary.ListStatus);
        Assert.Equal("2 von 5 Mods aktuell · 1 fehlt · 1 veraltet · 1 beschädigt", summary.Text);
    }

    [Fact]
    public void Summary_of_a_current_list()
    {
        var summary = ModSummary.From([Mod("A", ModState.Current)]);

        Assert.Equal("aktuell", summary.ListStatus);
        Assert.Equal("Alles aktuell: 1 Mods", summary.Text);
    }

    [Fact]
    public void Open_mods_come_first()
    {
        var sorted = ModSummary.Sort([Mod("A", ModState.Current), Mod("Z", ModState.Missing), Mod("B", ModState.Newer)]);

        Assert.Equal(["B", "Z", "A"], sorted.Select(mod => mod.Required.Name));
    }

    [Fact]
    public void Cleanup_summary_names_what_could_not_be_deleted()
    {
        var gigabyte = 1024L * 1024 * 1024;
        var result = new CleanupResult(
            [new SurplusMod("Alt", @"D:\Mods\addons\Alt", gigabyte), new SurplusMod("Älter", @"D:\Mods\addons\Aelter", gigabyte / 2)],
            [new SurplusMod("Gesperrt", @"D:\Mods\addons\Gesperrt", 1)]);

        Assert.Equal($"2 überzählige Mods gelöscht ({1.5:0.0} GB frei). Nicht löschbar: Gesperrt.", result.Summary);
    }
}
