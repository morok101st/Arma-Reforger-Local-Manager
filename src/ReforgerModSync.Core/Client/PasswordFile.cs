using ReforgerModSync.Core.Addons;

namespace ReforgerModSync.Core.Client;

/// <summary>
/// Übergabe des Server-Passworts an die Hilfs-Mod. Bewusst nicht als Startparameter, weil der Client seine
/// Startparameter ins Log schreibt. Die Hilfs-Mod liest die Datei aus <c>$profile:</c> und löscht sie dabei.
/// </summary>
public static class PasswordFile
{
    public const string FileName = "rms_server_password.txt";

    /// <summary>Im eigenen Profil des Clients, das Liste und Download nutzen.</summary>
    public static string ForClientProfile(string clientProfileDir) => Path.Combine(clientProfileDir, "profile", FileName);

    /// <summary>Im echten Profil des Spielers, das „Direkt beitreten“ nutzt.</summary>
    public static string ForPlayerProfile { get; } = Path.Combine(AddonPaths.ClientDefaultFolder, "profile", FileName);

    /// <summary>
    /// Entfernt immer zuerst eine alte Datei und schreibt nur bei einem Passwort eine neue. Sonst fände die
    /// Hilfs-Mod das Passwort eines anderen Servers, etwa nach einem Absturz oder einem Beitritt kurz davor.
    /// </summary>
    public static void Prepare(string path, string? password)
    {
        // File.Delete scheitert, wenn schon der Ordner fehlt.
        if (File.Exists(path))
        {
            File.Delete(path);
        }

        if (string.IsNullOrEmpty(password))
        {
            return;
        }

        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, password);
    }

    /// <summary>Absicherung, falls die Hilfs-Mod nicht zum Lesen kommt. Scheitert das Löschen, folgt der nächste Versuch beim nächsten Start.</summary>
    /// <returns><c>true</c>, wenn die Datei danach nicht mehr existiert.</returns>
    public static bool TryDelete(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }

            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    /// <summary>Beim App-Start: Reste aus einem abgestürzten Lauf an beiden Orten entfernen.</summary>
    public static void CleanUpLeftovers(AppPaths paths)
    {
        TryDelete(ForClientProfile(paths.ClientProfile));
        TryDelete(ForPlayerProfile);
    }
}
