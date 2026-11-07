using System.ComponentModel;
using System.Diagnostics;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Client;

/// <summary>
/// Startet das Spiel zum Spielen, mit dem echten Profil des Spielers und über den BattlEye-Starter,
/// wie es die Installation des Spiels selbst vorsieht. Es läuft bewusst unabhängig von der App weiter.
/// </summary>
public static class GameLauncher
{
    private const string BattlEyeStarter = "ArmaReforger_BE.exe";
    private const string PasswordFileName = "rms_server_password.txt";

    public static string PasswordFile => Path.Combine(AddonPaths.ClientDefaultFolder, "profile", PasswordFileName);

    /// <summary>Startet das Spiel und lässt die Hilfs-Mod dem Server beitreten.</summary>
    /// <returns><c>null</c> bei Erfolg, sonst eine Meldung für den Nutzer.</returns>
    public static string? StartAndJoin(AppSettings settings, ServerProfile profile, string? password)
    {
        if (!File.Exists(Path.Combine(AddonPaths.HelperRoot, "ReforgerModSyncHelper", "addon.gproj")))
        {
            return "Die Hilfs-Mod fehlt neben der App. Bitte die App neu installieren.";
        }

        // Bewusst nicht als Startparameter: Der Client schreibt seine Startparameter ins Log.
        if (!string.IsNullOrEmpty(password))
        {
            Directory.CreateDirectory(Path.GetDirectoryName(PasswordFile)!);
            File.WriteAllText(PasswordFile, password);
        }

        var problem = Launch(settings, ClientArgsBuilder.BuildJoin(profile.ServerAddress!, profile.DownloadDir, AddonPaths.HelperRoot));
        if (problem is not null)
        {
            DeletePasswordFile();
        }

        return problem;
    }

    /// <summary>Die Hilfs-Mod löscht die Datei beim Lesen. Das hier ist die Absicherung, falls sie nicht dazu kommt.</summary>
    public static void DeletePasswordFile()
    {
        try
        {
            File.Delete(PasswordFile);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // Wird beim nächsten App-Start erneut versucht.
        }
    }

    /// <summary>Wartet, bis die Datei weg ist, längstens <paramref name="maxWait"/>. Blockiert, nur für das App-Ende gedacht.</summary>
    /// <returns><c>true</c>, wenn die Datei nicht mehr existiert.</returns>
    public static bool WaitUntilRemoved(string path, TimeSpan maxWait)
    {
        var stopwatch = Stopwatch.StartNew();
        while (File.Exists(path))
        {
            if (stopwatch.Elapsed >= maxWait)
            {
                return false;
            }

            Thread.Sleep(TimeSpan.FromMilliseconds(100));
        }

        return true;
    }

    private static string? Launch(AppSettings settings, IReadOnlyList<string> arguments)
    {
        if (!SteamLocator.IsSteamRunning())
        {
            return "Bitte zuerst Steam starten.";
        }

        if (SteamLocator.IsGameRunning())
        {
            return "Arma Reforger läuft bereits. Bitte das Spiel zuerst schließen.";
        }

        var exe = SteamLocator.ResolveClientExe(settings.ReforgerExe);
        if (exe is null || !File.Exists(exe))
        {
            return "Arma Reforger wurde nicht gefunden.";
        }

        var folder = Path.GetDirectoryName(exe)!;
        var starter = Path.Combine(folder, BattlEyeStarter);
        var useStarter = File.Exists(starter);
        IEnumerable<string> all = useStarter ? ["-exe", Path.GetFileName(exe), .. arguments] : arguments;

        // ShellExecute, weil der BattlEye-Starter Administratorrechte anfordern kann.
        var startInfo = new ProcessStartInfo(useStarter ? starter : exe)
        {
            UseShellExecute = true,
            WorkingDirectory = folder,
            Arguments = ClientArgsBuilder.ToCommandLine(all),
        };

        try
        {
            using var process = Process.Start(startInfo);
            return null;
        }
        catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
        {
            return $"Das Spiel konnte nicht gestartet werden: {ex.Message}";
        }
    }
}
