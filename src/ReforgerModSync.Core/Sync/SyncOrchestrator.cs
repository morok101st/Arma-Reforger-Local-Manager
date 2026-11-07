using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Credentials;
using ReforgerModSync.Core.Settings;

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
/// <param name="paths">Ablageorte der App. <c>null</c>: <see cref="AppPaths.Default"/>.</param>
/// <param name="startClient">Startet den Client. <c>null</c>: <see cref="ClientSession.Start"/>.</param>
public sealed class SyncOrchestrator(
    ICredentialStore credentialStore,
    ModListCache cache,
    AppPaths? paths = null,
    ClientStarter? startClient = null,
    ServerListReader? listReader = null,
    DownloadWatcher? watcher = null)
{
    private const long MinimumFreeBytes = 2L * 1024 * 1024 * 1024;

    private readonly AppPaths _paths = paths ?? AppPaths.Default;
    private readonly ClientStarter _startClient = startClient ?? ClientSession.Start;
    private readonly ServerListReader _listReader = listReader ?? ServerListReader.Default;
    private readonly DownloadWatcher _watcher = watcher ?? DownloadWatcher.Default;

    /// <summary>Alles, was ein Lauf durchreicht. Hält die Parameterlisten kurz.</summary>
    private sealed record SyncRun(
        AppSettings Settings,
        ServerProfile Profile,
        string ClientExe,
        IProgress<SyncStatus> Progress,
        CancellationToken Token)
    {
        public string Address => Profile.ServerAddress!.Trim();

        public string ModsRoot => AddonPaths.ResolveModsRoot(Profile.DownloadDir);
    }

    /// <summary>Holt nur die Liste und vergleicht.</summary>
    public Task<SyncResult> CheckAsync(
        AppSettings settings,
        ServerProfile profile,
        IProgress<SyncStatus> progress,
        CancellationToken cancellationToken) =>
        RunExclusiveAsync(async () =>
        {
            if (ClientPrecheck.Check(settings, profile, out var clientExe) is { } problem)
            {
                return Failed(problem);
            }

            var run = new SyncRun(settings, profile, clientExe, progress, cancellationToken);
            var required = await FetchListAsync(run);
            var comparison = await CompareAsync(required, run.ModsRoot, cancellationToken);
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
            if (ClientPrecheck.Check(settings, profile, out var clientExe) is { } problem)
            {
                return Failed(problem);
            }

            var run = new SyncRun(settings, profile, clientExe, progress, cancellationToken);
            var required = await FetchListAsync(run);
            return await DownloadAsync(run, required);
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
            // Nicht auf dem Thread des Aufrufers: Die Überwachung liest laufend Mod-Ordner und würde die
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
        catch (InvalidOperationException ex)
        {
            // Etwa wenn Windows den Client nicht starten konnte.
            return Failed($"Der Spielclient konnte nicht gestartet werden: {ex.Message}");
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

    private async Task<IReadOnlyList<ModEntry>> FetchListAsync(SyncRun run)
    {
        run.Progress.Report(new SyncStatus("Mod-Liste wird vom Server geholt …"));
        var (logsDir, profileDir) = PrepareRunFolders();
        var arguments = ClientArgsBuilder.BuildListRequest(run.Address, run.Profile.DownloadDir, run.Settings.Client, logsDir, profileDir, AddonPaths.HelperRoot);
        var mods = await WithClientAsync(run, arguments, profileDir, client => _listReader.ReadAsync(client, logsDir, run.Address, run.Token));
        await cache.SaveAsync(run.Profile.Id, mods, run.Token);
        return mods;
    }

    private async Task<SyncResult> DownloadAsync(SyncRun run, IReadOnlyList<ModEntry> required)
    {
        var modsRoot = run.ModsRoot;
        var comparison = await CompareAsync(required, modsRoot, run.Token);
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

        var deadline = DateTime.UtcNow.AddMinutes(run.Settings.Timeouts.TotalMinutes);
        var failures = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

        var (end, logsDir) = await DownloadOnceAsync(run, required, comparison, deadline, failures);
        if (end == WatchEnd.Idle)
        {
            run.Progress.Report(new SyncStatus("Keine Aktivität mehr, der Client wird einmal neu gestartet …"));
            comparison = await CompareAsync(required, modsRoot, run.Token);
            (end, logsDir) = await DownloadOnceAsync(run, required, comparison, deadline, failures);
        }

        comparison = await CompareAsync(required, modsRoot, run.Token);
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

    /// <summary>Ein Start des Clients für alle noch offenen Mods.</summary>
    private async Task<(WatchEnd End, string LogsDir)> DownloadOnceAsync(
        SyncRun run,
        IReadOnlyList<ModEntry> required,
        IReadOnlyList<ModComparison> comparison,
        DateTime deadline,
        Dictionary<string, string> failures)
    {
        var open = comparison.Where(mod => mod.State != ModState.Current).Select(mod => mod.Required).ToList();
        run.Progress.Report(new SyncStatus($"Spielclient lädt {open.Count} Mods …"));
        var (logsDir, profileDir) = PrepareRunFolders();
        var arguments = ClientArgsBuilder.BuildDownload(run.Address, open, run.Profile.DownloadDir, run.Settings.Client, logsDir, profileDir, AddonPaths.HelperRoot);
        var watch = new DownloadWatch(
            required,
            run.ModsRoot,
            logsDir,
            TimeSpan.FromMinutes(run.Settings.Timeouts.IdleMinutes),
            deadline,
            failures,
            run.Progress);
        var end = await WithClientAsync(run, arguments, profileDir, client => _watcher.WatchAsync(client, watch, run.Token));
        return (end, logsDir);
    }

    /// <summary>
    /// Schreibt die Passwortdatei, startet den Client und räumt danach in jedem Fall auf: Client beenden,
    /// Passwortdatei löschen. Die Hilfs-Mod löscht die Datei schon beim Lesen, das hier ist die Absicherung.
    /// </summary>
    private async Task<T> WithClientAsync<T>(
        SyncRun run,
        IReadOnlyList<string> arguments,
        string profileDir,
        Func<IClientProcess, Task<T>> work)
    {
        var passwordFile = PasswordFile.ForClientProfile(profileDir);
        PasswordFile.Prepare(passwordFile, credentialStore.ReadPassword(run.Address, run.Profile.Id));
        try
        {
            using var client = _startClient(run.ClientExe, arguments, run.Settings.Client.HideWindow);
            try
            {
                return await work(client);
            }
            finally
            {
                await client.StopAsync();
            }
        }
        finally
        {
            PasswordFile.TryDelete(passwordFile);
        }
    }

    private (string LogsDir, string ProfileDir) PrepareRunFolders()
    {
        var logsDir = ClientLogRetention.CreateRunFolder(_paths.ClientLogs);
        Directory.CreateDirectory(_paths.ClientProfile);
        return (logsDir, _paths.ClientProfile);
    }

    private static async Task<IReadOnlyList<ModComparison>> CompareAsync(
        IReadOnlyList<ModEntry> required,
        string modsRoot,
        CancellationToken cancellationToken) =>
        AddonComparer.Compare(required, await AddonScanner.ScanAsync(modsRoot, cancellationToken));

    private static SyncResult Failed(string message) => new(SyncOutcome.Failed, message, []);
}
