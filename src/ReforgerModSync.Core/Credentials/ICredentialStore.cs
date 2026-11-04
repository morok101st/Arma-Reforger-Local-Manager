namespace ReforgerModSync.Core.Credentials;

/// <summary>
/// Server-Passwörter gehören zu einem Profil und seiner Adresse: Sie werden nie für ein anderes
/// Profil und ohne Zustimmung des Nutzers nie für eine andere Adresse verwendet.
/// </summary>
public interface ICredentialStore
{
    string? ReadPassword(string serverAddress, string profileId);
    void SavePassword(string serverAddress, string profileId, string password);

    /// <returns><c>true</c>, wenn ein Eintrag gelöscht wurde.</returns>
    bool DeletePassword(string serverAddress, string profileId);
}
