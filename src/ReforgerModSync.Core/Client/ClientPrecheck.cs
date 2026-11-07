using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Client;

/// <summary>Gemeinsame Vorprüfung vor jedem Start des Spielclients: Sync, Prüfen und Beitritt.</summary>
public static class ClientPrecheck
{
    public static bool IsHelperInstalled =>
        File.Exists(Path.Combine(AddonPaths.HelperRoot, "ReforgerModSyncHelper", "addon.gproj"));

    /// <returns><c>null</c>, wenn der Client starten darf. Sonst eine Meldung für den Nutzer.</returns>
    public static string? Check(AppSettings settings, ServerProfile profile, out string clientExe)
    {
        clientExe = "";
        if (!ClientArgsBuilder.IsValidServerAddress(profile.ServerAddress))
        {
            return "Es ist keine gültige Server-Adresse hinterlegt (IP:Port).";
        }

        if (!IsHelperInstalled)
        {
            return "Die Hilfs-Mod fehlt neben der App. Bitte die App neu installieren.";
        }

        if (!SteamLocator.IsSteamRunning())
        {
            return "Bitte zuerst Steam starten.";
        }

        if (SteamLocator.IsGameRunning())
        {
            return "Arma Reforger läuft bereits. Bitte das Spiel zuerst schließen.";
        }

        // Auch hier und nicht nur im Dialog: Die Einstellungsdatei könnte von Hand geändert worden sein.
        if (settings.ReforgerExe is { } configured && SteamLocator.CheckClientExe(configured) is { } exeProblem)
        {
            return $"Der Spielclient in den Grundeinstellungen ist ungültig: {exeProblem}";
        }

        var exe = SteamLocator.ResolveClientExe(settings.ReforgerExe);
        if (exe is null || !File.Exists(exe))
        {
            return "Arma Reforger wurde nicht gefunden. Läuft die Erkennung über Steam ins Leere, den Spielclient in den Grundeinstellungen eintragen.";
        }

        clientExe = exe;
        return null;
    }
}
