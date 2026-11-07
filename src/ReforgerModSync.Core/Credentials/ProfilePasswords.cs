namespace ReforgerModSync.Core.Credentials;

/// <summary>
/// Regeln für das Passwort eines Servers. Es gilt nur für seine Adresse: Für eine andere darf es nur mit
/// Zustimmung des Nutzers verwendet werden, sonst ginge es unbemerkt an einen fremden Server.
/// </summary>
public sealed class ProfilePasswords(ICredentialStore store)
{
    public bool HasPassword(string? address, string profileId) =>
        address is not null && store.ReadPassword(address, profileId) is not null;

    /// <summary>
    /// Muss der Nutzer entscheiden, ob das gespeicherte Passwort für die neue Adresse gelten soll? Nur wenn er kein
    /// neues eingibt, eines gespeichert ist und sich die Adresse tatsächlich ändert.
    /// </summary>
    public static bool NeedsTransferDecision(bool hasSavedPassword, string? savedAddress, string? newAddress, string enteredPassword) =>
        enteredPassword.Length == 0
        && hasSavedPassword
        && savedAddress is not null
        && newAddress is not null
        && newAddress != savedAddress;

    /// <summary>Speichert ein neu eingegebenes Passwort. Ein leeres lässt das gespeicherte unverändert.</summary>
    public void StoreEntered(string address, string profileId, string enteredPassword)
    {
        if (enteredPassword.Length > 0)
        {
            store.SavePassword(address, profileId, enteredPassword);
        }
    }

    /// <summary>
    /// Die Adresse hat sich geändert. Mit <paramref name="keep"/> wandert das Passwort zur neuen Adresse. Der Eintrag
    /// der alten Adresse verschwindet in jedem Fall, damit kein verwaistes Passwort zurückbleibt.
    /// </summary>
    public void MoveToNewAddress(string profileId, string previousAddress, string newAddress, bool keep)
    {
        if (keep && store.ReadPassword(previousAddress, profileId) is { } password)
        {
            store.SavePassword(newAddress, profileId, password);
        }

        store.DeletePassword(previousAddress, profileId);
    }
}
