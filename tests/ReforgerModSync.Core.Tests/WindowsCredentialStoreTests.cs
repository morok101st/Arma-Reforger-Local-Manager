using ReforgerModSync.Core.Credentials;

namespace ReforgerModSync.Core.Tests;

/// <summary>Nutzt den echten Anmeldeinformationsspeicher mit eindeutigen Test-IDs und räumt danach auf.</summary>
public sealed class WindowsCredentialStoreTests : IDisposable
{
    private const string Address = "203.0.113.10:2001";
    private const string OtherAddress = "203.0.113.20:2001";

    private readonly WindowsCredentialStore _store = new();
    private readonly string _profile = $"test-{Guid.NewGuid():N}";
    private readonly string _otherProfile = $"test-{Guid.NewGuid():N}";

    public void Dispose()
    {
        _store.DeletePassword(Address, _profile);
        _store.DeletePassword(Address, _otherProfile);
        _store.DeletePassword(OtherAddress, _profile);
    }

    [Fact]
    public void Saved_password_can_be_read_and_deleted()
    {
        _store.SavePassword(Address, _profile, "pässwört-€-123");

        Assert.Equal("pässwört-€-123", _store.ReadPassword(Address, _profile));
        Assert.True(_store.DeletePassword(Address, _profile));
        Assert.Null(_store.ReadPassword(Address, _profile));
        Assert.False(_store.DeletePassword(Address, _profile));
    }

    [Fact]
    public void Two_profiles_on_the_same_server_have_separate_passwords()
    {
        _store.SavePassword(Address, _profile, "geheim-a");
        _store.SavePassword(Address, _otherProfile, "geheim-b");

        Assert.Equal("geheim-a", _store.ReadPassword(Address, _profile));
        Assert.Equal("geheim-b", _store.ReadPassword(Address, _otherProfile));
    }

    [Fact]
    public void Password_is_not_used_for_another_address()
    {
        _store.SavePassword(Address, _profile, "geheim");

        Assert.Null(_store.ReadPassword(OtherAddress, _profile));
    }
}
