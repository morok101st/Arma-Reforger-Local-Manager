using System.ComponentModel;
using System.Diagnostics;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.Core.Client;

/// <param name="Problem">Meldung für den Nutzer, <c>null</c> bei Erfolg.</param>
/// <param name="PasswordFile">Die Passwortdatei, die der Aufrufer später sicherheitshalber löscht. <c>null</c>: keine geschrieben.</param>
public sealed record JoinStart(string? Problem, string? PasswordFile);

/// <summary>
/// Startet das Spiel zum Spielen, mit dem echten Profil des Spielers und über den BattlEye-Starter,
/// wie es die Installation des Spiels selbst vorsieht. Es läuft bewusst unabhängig von der App weiter.
/// </summary>
public static class GameLauncher
{
    private const string BattlEyeStarter = "ArmaReforger_BE.exe";

    /// <summary>
    /// Startet das Spiel und lässt die Hilfs-Mod dem Server beitreten. Die Logs dieser Sitzung und die
    /// Passwortdatei liegen in einem eigenen Lauf-Ordner der App, nicht in „Dokumente“.
    /// </summary>
    public static JoinStart StartAndJoin(AppSettings settings, ServerProfile profile, string? password, AppPaths paths)
    {
        // Erst alles prüfen, dann die Passwortdatei schreiben: Sie soll nur existieren, wenn das Spiel auch startet.
        if (ClientPrecheck.Check(settings, profile, out var exe) is { } problem)
        {
            return new JoinStart(problem, null);
        }

        var logsDir = ClientLogRetention.CreateRunFolder(paths.ClientLogs);
        var passwordFile = PasswordFile.ForLogsFolder(logsDir);
        PasswordFile.Prepare(passwordFile, password);

        // Die Hilfs-Mod liest zur Not auch noch den früheren Ort. Dort darf nie ein altes Passwort liegen.
        PasswordFile.TryDelete(PasswordFile.LegacyPlayerProfile);

        problem = Launch(exe, ClientArgsBuilder.BuildJoin(profile.ServerAddress!, profile.DownloadDir, AddonPaths.HelperRoot, logsDir));
        if (problem is not null)
        {
            PasswordFile.TryDelete(passwordFile);
            return new JoinStart(problem, null);
        }

        return new JoinStart(null, string.IsNullOrEmpty(password) ? null : passwordFile);
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

    private static string? Launch(string exe, IReadOnlyList<string> arguments)
    {
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
