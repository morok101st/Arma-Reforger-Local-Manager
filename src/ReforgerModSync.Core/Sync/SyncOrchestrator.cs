using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Credentials;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;

namespace ReforgerModSync.Core.Sync;

public enum SyncOutcome
{
    /// <summary>Nur geprüft: Die Liste kam vom Server, es wurde nichts geladen.</summary>
    Checked,
    AlreadyCurrent,
    Completed,
    Incomplete,
    Cancelled,
    Failed,
}

public sealed record SyncStatus(string Message, int Done = 0, int Total = 0, IReadOnlyList<ModComparison>? Mods = null);

public sealed record SyncResult(SyncOutcome Outcome, string Message, IReadOnlyList<ModComparison> Mods);

/// <summary>
/// Holt die Mod-Liste über die Hilfs-Mod aus dem Server-Eintrag im Backend, vergleicht sie mit dem
/// Zielordner und lädt die offenen Mods. Dafür läuft der Spielclient zweimal kurz. Es entsteht
/// keine Verbindung zum Spielserver und kein Beitritt.
/// </summary>
public sealed class SyncOrchestrator(ICredentialStore credentialStore, ModListCache cache)
{
    private const long MinimumFreeBytes = 2L * 1024 * 1024 * 1024;
    private const string PasswordFileName = "rms_server_password.txt";
    private static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(3);
    private static readonly TimeSpan ListPollInterval = TimeSpan.FromMilliseconds(500);
    private static readonly TimeSpan ListTimeout = TimeSpan.FromMinutes(2);

    private enum WatchEnd
    {
        AllCurrent,
        HelperDone,
        ClientExited,
        Idle,
        TotalTimeout,
    }

    /// <summary>Holt nur die Liste und vergleicht.</summary>
    public Task<SyncResult> CheckAsync(
        AppSettings settings,
        ServerProfile profile,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken) =>
        RunExclusiveAsync(async () =>
        {
            if (Precheck(settings, profile, out var clientExe) is { } problem)
            {
                return Failed(problem);
            }

            var required = await FetchListAsync(settings, profile, clientExe, progress, cancellationToken);
            var comparison = await CompareAsync(required, AddonPaths.ResolveModsRoot(profile.DownloadDir), cancellationToken);
            return new SyncResult(SyncOutcome.Checked, $"Liste vom Server geholt: {comparison.Count} Mods.", comparison);
        }, cancellationToken);

    /// <summary>Fehler führen nie zu Ausnahmen, sondern zu einem <see cref="SyncResult"/> mit klarer Meldung.</summary>
    public Task<SyncResult> RunAsync(
        AppSettings settings,
        ServerProfile profile,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken) =>
        RunExclusiveAsync(async () =>
        {
            progress.Report(new SyncStatus("Vorprüfung …"));
            if (Precheck(settings, profile, out var clientExe) is { } problem)
            {
                return Failed(problem);
            }

            var required = await FetchListAsync(settings, profile, clientExe, progress, cancellationToken);
            return await DownloadAsync(settings, profile, clientExe, required, progress, cancellationToken);
        }, cancellationToken);

    /// <summary>Es kann immer nur ein Client laufen, die Sperre gilt deshalb profilübergreifend.</summary>
    private static async Task<SyncResult> RunExclusiveAsync(Func<Task<SyncResult>> run, CancellationToken cancellationToken)
    {
        // Named Semaphore statt Mutex: Ein Mutex wäre an den Thread gebunden, der Lauf wechselt aber Threads.
        using var runLock = new Semaphore(1, 1, @"Local\ReforgerModSync.Sync");
        if (!runLock.WaitOne(0))
        {
            return Failed("Es läuft bereits eine Synchronisierung.");
        }

        try
        {
            // Nicht auf dem Thread des Aufrufers: Die Überwachung liest laufend große Mod-Ordner und würde die
            // Oberfläche und damit auch das Abbrechen ausbremsen. Fortschritt meldet IProgress an die Oberfläche.
            return await Task.Run(run, CancellationToken.None);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            return new SyncResult(SyncOutcome.Cancelled, "Abgebrochen. Bereits geladene Mods bleiben erhalten.", []);
        }
        catch (ServerListException ex)
        {
            return Failed(ex.Message);
        }
        catch (Exception ex) when (ex is ArgumentException or IOException or UnauthorizedAccessException
            or System.ComponentModel.Win32Exception)
        {
            return Failed(ex.Message);
        }
        finally
        {
            runLock.Release();
        }
    }

    private async Task<IReadOnlyList<ModEntry>> FetchListAsync(
        AppSettings settings,
        ServerProfile profile,
        string clientExe,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken)
    {
        progress.Report(new SyncStatus("Mod-Liste wird vom Server geholt …"));
        var address = profile.ServerAddress!.Trim();
        var (logsDir, profileDir) = PrepareRunFolders();
        var passwordFile = await WritePasswordFileAsync(profile, profileDir, cancellationToken);

        try
        {
            var arguments = ClientArgsBuilder.BuildListRequest(address, profile.DownloadDir, settings.Client, logsDir, profileDir, AddonPaths.HelperRoot);
            using var session = ClientSession.Start(clientExe, arguments, settings.Client.HideWindow);
            try
            {
                var mods = await ReadListAsync(session, logsDir, address, cancellationToken);
                await cache.SaveAsync(profile.Id, mods, cancellationToken);
                return mods;
            }
            finally
            {
                await session.StopAsync();
            }
        }
        finally
        {
            File.Delete(passwordFile);
        }
    }

    /// <summary>Bewusst nicht als Startparameter: Der Client schreibt seine Startparameter ins Log.</summary>
    /// <returns>Der Pfad, den der Aufrufer am Ende löscht. Die Hilfs-Mod löscht die Datei schon beim Lesen.</returns>
    private async Task<string> WritePasswordFileAsync(
        ServerProfile profile,
        string profileDir,
        CancellationToken cancellationToken)
    {
        var password = credentialStore.ReadPassword(profile.ServerAddress!.Trim(), profile.Id);
        var profileData = Path.Combine(profileDir, "profile");
        Directory.CreateDirectory(profileData);
        var passwordFile = Path.Combine(profileData, PasswordFileName);
        if (!string.IsNullOrEmpty(password))
        {
            await File.WriteAllTextAsync(passwordFile, password, cancellationToken);
        }

        return passwordFile;
    }

    private static async Task<IReadOnlyList<ModEntry>> ReadListAsync(
        ClientSession session,
        string logsDir,
        string address,
        CancellationToken cancellationToken)
    {
        var mods = new List<ModEntry>();
        var tail = new LogTail(Path.Combine(logsDir, "console.log"));
        var deadline = DateTime.UtcNow + ListTimeout;
        while (DateTime.UtcNow < deadline)
        {
            await Task.Delay(ListPollInterval, cancellationToken);

            // Erst den Prozessstand lesen: Beendet er sich gerade, stehen seine letzten Zeilen schon im Log.
            var exited = session.HasExited;
            foreach (var line in tail.ReadNewLines())
            {
                if (ClientLogParser.ParseServerMod(line) is { } mod)
                {
                    mods.Add(mod);
                }

                switch (ClientLogParser.ParseServerListEnd(line))
                {
                    case ServerListEnd.Done:
                        // Die Liste entscheidet auch darüber, was gelöscht wird. Eine unvollständige darf nie gelten.
                        if (ClientLogParser.ParseListDoneCount(line) is { } expected && expected != mods.Count)
                        {
                            throw new ServerListException($"Die Mod-Liste kam unvollständig an ({mods.Count} von {expected} Mods). Bitte erneut versuchen. Client-Logs: {logsDir}");
                        }

                        return mods;
                    case ServerListEnd.NotFound:
                        throw new ServerListException($"Der Server {address} wurde nicht gefunden. Läuft er, und stimmt die Adresse?");
                    case ServerListEnd.PasswordRequired:
                        throw new ServerListException("Der Server verlangt ein Passwort. Bitte in den Einstellungen eintragen.");
                    case ServerListEnd.PasswordRejected:
                        throw new ServerListException("Der Server hat das Passwort abgelehnt. Bitte in den Einstellungen prüfen.");
                    case ServerListEnd.Failed:
                        throw new ServerListException("Die Mod-Liste konnte nicht geladen werden. Bitte später erneut versuchen.");
                }
            }

            if (exited)
            {
                var crashed = File.Exists(Path.Combine(logsDir, "crash.log"));
                throw new ServerListException(crashed
                    ? $"Der Spielclient ist abgestürzt. Client-Logs: {logsDir}"
                    : $"Der Spielclient wurde beendet, bevor die Liste da war. Client-Logs: {logsDir}");
            }
        }

        throw new ServerListException($"Die Mod-Liste kam nicht innerhalb von {ListTimeout.TotalMinutes:0} Minuten an. Client-Logs: {logsDir}");
    }

    private async Task<SyncResult> DownloadAsync(
        AppSettings settings,
        ServerProfile profile,
        string clientExe,
        IReadOnlyList<ModEntry> required,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken)
    {
        var modsRoot = AddonPaths.ResolveModsRoot(profile.DownloadDir);
        var comparison = await CompareAsync(required, modsRoot, cancellationToken);
        if (comparison.All(mod => mod.State == ModState.Current))
        {
            return new SyncResult(SyncOutcome.AlreadyCurrent, "Alles aktuell.", comparison);
        }

        Directory.CreateDirectory(modsRoot);
        var freeBytes = new DriveInfo(Path.GetFullPath(modsRoot)).AvailableFreeSpace;
        if (freeBytes < MinimumFreeBytes)
        {
            return Failed($"Zu wenig freier Speicher im Zielordner ({freeBytes / (1024 * 1024)} MB frei).");
        }

        var (logsDir, profileDir) = PrepareRunFolders();
        var deadline = DateTime.UtcNow.AddMinutes(settings.Timeouts.TotalMinutes);
        var failures = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

        var end = await RunClientAsync(comparison, settings, profile, clientExe, required, modsRoot, logsDir, profileDir, deadline, failures, progress, cancellationToken);
        if (end == WatchEnd.Idle)
        {
            progress.Report(new SyncStatus("Keine Aktivität mehr, der Client wird einmal neu gestartet …"));
            comparison = await CompareAsync(required, modsRoot, cancellationToken);
            (logsDir, profileDir) = PrepareRunFolders();
            end = await RunClientAsync(comparison, settings, profile, clientExe, required, modsRoot, logsDir, profileDir, deadline, failures, progress, cancellationToken);
        }

        comparison = await CompareAsync(required, modsRoot, cancellationToken);
        var remaining = comparison.Where(mod => mod.State != ModState.Current).ToList();
        if (remaining.Count == 0)
        {
            return new SyncResult(SyncOutcome.Completed, $"Fertig: alle {comparison.Count} Mods sind aktuell.", comparison);
        }

        var reason = end switch
        {
            WatchEnd.ClientExited when Directory.EnumerateFiles(logsDir, "crash.log", SearchOption.AllDirectories).Any() =>
                "Der Spielclient ist abgestürzt.",
            WatchEnd.ClientExited => "Der Spielclient hat sich vorzeitig beendet.",
            WatchEnd.Idle => "Der Download zeigt keine Aktivität mehr.",
            WatchEnd.TotalTimeout => "Die maximale Laufzeit wurde erreicht.",
            _ => "Der Download wurde nicht abgeschlossen.",
        };
        var names = string.Join(", ", remaining.Take(5).Select(mod =>
            failures.TryGetValue(mod.Required.Id, out var failure) ? $"{mod.Required.Name} ({failure})" : mod.Required.Name));
        var more = remaining.Count > 5 ? $" und {remaining.Count - 5} weitere" : "";
        return new SyncResult(
            SyncOutcome.Incomplete,
            $"{remaining.Count} Mods fehlen noch: {names}{more}. {reason} Client-Logs: {logsDir}",
            comparison);
    }

    private static (string LogsDir, string ProfileDir) PrepareRunFolders()
    {
        var dataDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "ReforgerModSync");
        var logsDir = Path.Combine(dataDir, "clientlogs", DateTime.Now.ToString("yyyyMMdd-HHmmss-fff"));
        var profileDir = Path.Combine(dataDir, "clientprofile");
        Directory.CreateDirectory(logsDir);
        Directory.CreateDirectory(profileDir);
        return (logsDir, profileDir);
    }

    private static string? Precheck(AppSettings settings, ServerProfile profile, out string clientExe)
    {
        clientExe = "";
        if (!ClientArgsBuilder.IsValidServerAddress(profile.ServerAddress))
        {
            return "Es ist keine gültige Server-Adresse hinterlegt (IP:Port).";
        }

        if (!SteamLocator.IsSteamRunning())
        {
            return "Bitte zuerst Steam starten.";
        }

        if (SteamLocator.IsGameRunning())
        {
            return "Arma Reforger läuft bereits. Bitte das Spiel zuerst schließen.";
        }

        var steamPath = SteamLocator.ReadSteamPathFromRegistry();
        var exe = settings.ReforgerExe ?? (steamPath is null ? null : SteamLocator.FindClientExe(steamPath));
        if (exe is null || !File.Exists(exe))
        {
            return "Arma Reforger wurde nicht gefunden.";
        }

        if (!File.Exists(Path.Combine(AddonPaths.HelperRoot, "ReforgerModSyncHelper", "addon.gproj")))
        {
            return "Die Hilfs-Mod fehlt neben der App. Bitte die App neu installieren.";
        }

        clientExe = exe;
        return null;
    }

    private async Task<WatchEnd> RunClientAsync(
        IReadOnlyList<ModComparison> comparison,
        AppSettings settings,
        ServerProfile profile,
        string clientExe,
        IReadOnlyList<ModEntry> required,
        string modsRoot,
        string logsDir,
        string profileDir,
        DateTime deadline,
        Dictionary<string, string> failures,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken)
    {
        var open = comparison.Where(mod => mod.State != ModState.Current).Select(mod => mod.Required).ToList();
        progress.Report(new SyncStatus($"Spielclient lädt {open.Count} Mods …"));
        var arguments = ClientArgsBuilder.BuildDownload(profile.ServerAddress!, open, profile.DownloadDir, settings.Client, logsDir, profileDir, AddonPaths.HelperRoot);
        var passwordFile = await WritePasswordFileAsync(profile, profileDir, cancellationToken);
        try
        {
            using var session = ClientSession.Start(clientExe, arguments, settings.Client.HideWindow);
            try
            {
                return await WatchAsync(session, required, modsRoot, logsDir, settings.Timeouts, deadline, failures, progress, cancellationToken);
            }
            finally
            {
                await session.StopAsync();
            }
        }
        finally
        {
            File.Delete(passwordFile);
        }
    }

    private static async Task<WatchEnd> WatchAsync(
        ClientSession session,
        IReadOnlyList<ModEntry> required,
        string modsRoot,
        string logsDir,
        TimeoutSettings timeouts,
        DateTime deadline,
        Dictionary<string, string> failures,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken)
    {
        var tail = new LogTail(Path.Combine(logsDir, "console.log"));
        var idleLimit = TimeSpan.FromMinutes(timeouts.IdleMinutes);
        var lastActivity = DateTime.UtcNow;
        var lastBytes = -1L;
        var lastDone = -1;
        var helperDone = false;

        while (true)
        {
            await Task.Delay(PollInterval, cancellationToken);

            foreach (var line in tail.ReadNewLines())
            {
                var logEvent = ClientLogParser.Classify(line);
                if (logEvent == ClientLogEvent.None)
                {
                    continue;
                }

                if (logEvent != ClientLogEvent.BackendError)
                {
                    lastActivity = DateTime.UtcNow;
                }

                if (ClientLogParser.ParseModFailed(line) is { } failure)
                {
                    failures[failure.Id] = failure.Reason;
                }

                helperDone |= logEvent == ClientLogEvent.HelperDone;
                progress.Report(new SyncStatus($"[client] {line}"));
            }

            var comparison = await CompareAsync(required, modsRoot, cancellationToken);
            var done = comparison.Count(mod => mod.State == ModState.Current);
            if (done != lastDone)
            {
                lastDone = done;
                lastActivity = DateTime.UtcNow;
                progress.Report(new SyncStatus($"{done} von {comparison.Count} Mods fertig", done, comparison.Count, comparison));
            }

            if (done == comparison.Count)
            {
                return WatchEnd.AllCurrent;
            }

            if (helperDone)
            {
                return WatchEnd.HelperDone;
            }

            if (session.HasExited)
            {
                return WatchEnd.ClientExited;
            }

            // Große Mods schreiben lange keine Logzeile, wachsende Dateien zählen deshalb als Aktivität.
            var bytes = DownloadedBytes(modsRoot) + DownloadedBytes(Path.Combine(Path.GetTempPath(), "addons"));
            if (bytes != lastBytes)
            {
                lastBytes = bytes;
                lastActivity = DateTime.UtcNow;
            }

            if (DateTime.UtcNow - lastActivity > idleLimit)
            {
                return WatchEnd.Idle;
            }

            if (DateTime.UtcNow > deadline)
            {
                return WatchEnd.TotalTimeout;
            }
        }
    }

    private static async Task<IReadOnlyList<ModComparison>> CompareAsync(
        IReadOnlyList<ModEntry> required,
        string modsRoot,
        CancellationToken cancellationToken) =>
        AddonComparer.Compare(required, await AddonScanner.ScanAsync(modsRoot, cancellationToken));

    private static long DownloadedBytes(string folder)
    {
        if (!Directory.Exists(folder))
        {
            return 0;
        }

        var options = new EnumerationOptions { RecurseSubdirectories = true, IgnoreInaccessible = true };
        return new DirectoryInfo(folder).EnumerateFiles("*", options).Sum(file => file.Length);
    }

    private static SyncResult Failed(string message) => new(SyncOutcome.Failed, message, []);

    private sealed class ServerListException(string message) : Exception(message);
}
