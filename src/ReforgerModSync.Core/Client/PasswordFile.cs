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

    /// <summary>Im Log-Ordner eines Laufs (<c>$logs:</c>). So liegt sie beim Beitritt nicht in „Dokumente“, das oft in OneDrive liegt.</summary>
    public static string ForLogsFolder(string logsDir) => Path.Combine(logsDir, FileName);

    /// <summary>Früherer Ort beim Beitritt, im echten Profil des Spielers. Wird nur noch aufgeräumt.</summary>
    public static string LegacyPlayerProfile { get; } = Path.Combine(AddonPaths.ClientDefaultFolder, "profile", FileName);

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

    /// <summary>Beim App-Start: Reste aus abgestürzten Läufen an allen Orten entfernen.</summary>
    public static void CleanUpLeftovers(AppPaths paths)
    {
        TryDelete(ForClientProfile(paths.ClientProfile));
        TryDelete(LegacyPlayerProfile);
        if (Directory.Exists(paths.ClientLogs))
        {
            foreach (var run in Directory.EnumerateDirectories(paths.ClientLogs))
            {
                TryDelete(ForLogsFolder(run));
            }
        }
    }
}
