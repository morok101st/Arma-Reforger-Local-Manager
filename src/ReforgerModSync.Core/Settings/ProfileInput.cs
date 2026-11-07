using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Settings;

/// <summary>Prüft die Eingaben eines Servers in den Einstellungen, bevor er gespeichert wird.</summary>
public static class ProfileInput
{
    /// <returns><c>null</c>, wenn gespeichert werden darf. Sonst die Meldung für den Nutzer.</returns>
    public static string? Validate(string? name, string? ip, string? port)
    {
        if (string.IsNullOrWhiteSpace(name))
        {
            return "Bitte einen Namen für diesen Server eintragen.";
        }

        if (!ClientArgsBuilder.IsValidIp(ip))
        {
            return "Bitte die IP-Adresse des Servers eintragen, zum Beispiel 203.0.113.10.";
        }

        if (!ClientArgsBuilder.IsValidPort(port))
        {
            return "Bitte den Port des Servers eintragen, eine Zahl von 1 bis 65535, zum Beispiel 2001.";
        }

        return null;
    }
}
