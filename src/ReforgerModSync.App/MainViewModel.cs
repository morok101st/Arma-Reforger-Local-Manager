using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Credentials;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Sync;
using Serilog;

namespace ReforgerModSync.App;

public sealed partial class MainViewModel(
    SettingsStore settingsStore,
    ICredentialStore credentialStore,
    ModListCache cache,
    SyncOrchestrator orchestrator) : ObservableObject
{
    private static readonly TimeSpan PasswordCleanupDelay = TimeSpan.FromMinutes(2);

    private readonly SemaphoreSlim _saveGate = new(1, 1);
    private AppSettings _settings = new();
    private CancellationTokenSource? _runCancel;
    private bool _initialized;
    private DateTime? _passwordCleanupDue;

    [ObservableProperty] private int _selectedTab;

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(RefreshCommand))]
    private ProfileViewModel? _selectedProfile;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(SyncButtonText), nameof(IsNotBusy))]
    [NotifyCanExecuteChangedFor(nameof(RefreshCommand))]
    private bool _isBusy;

    public ObservableCollection<ProfileViewModel> Profiles { get; } = [];

    public string SyncButtonText => IsBusy ? "Abbrechen" : "Jetzt synchronisieren";
    public bool IsNotBusy => !IsBusy;

    /// <summary>Wie lange die App nach einem Beitritt noch auf das Löschen der Passwortdatei achten muss.</summary>
    public TimeSpan PasswordCleanupWait => _passwordCleanupDue is { } due && due > DateTime.UtcNow
        ? due - DateTime.UtcNow
        : TimeSpan.Zero;

    /// <summary>Beim Start läuft kein Spielclient: Die Übersicht nutzt die zuletzt geholte Liste.</summary>
    public async Task InitializeAsync()
    {
        string? loadProblem = null;
        try
        {
            _settings = await settingsStore.LoadAsync();
        }
        catch (SettingsException ex)
        {
            loadProblem = ex.Message + " Beim Speichern wird sie ersetzt.";
        }

        foreach (var saved in _settings.Profiles)
        {
            var profile = new ProfileViewModel(saved);
            ReadPasswordState(profile);
            await ShowCachedListAsync(profile);
            Profiles.Add(profile);
        }

        if (Profiles.Count == 0)
        {
            Profiles.Add(new ProfileViewModel());
        }

        SelectedProfile = Profiles.FirstOrDefault(p => p.Id == _settings.SelectedProfileId) ?? Profiles[0];
        _initialized = true;

        if (loadProblem is not null)
        {
            ShowSettingsMessage(SelectedProfile, loadProblem, isError: true);
        }
        else if (!IsConfigured(SelectedProfile))
        {
            ShowSettingsMessage(SelectedProfile, "Bitte Adresse und Passwort des Spielservers eintragen.", isError: false);
        }

        if (!IsConfigured(SelectedProfile))
        {
            SelectedTab = 1;
        }
    }

    [RelayCommand]
    private void AddProfile()
    {
        var profile = new ProfileViewModel();
        Profiles.Add(profile);
        SelectedProfile = profile;
        SelectedTab = 1;
        ShowSettingsMessage(profile, "Bitte Adresse und Passwort des Spielservers eintragen.", isError: false);
    }

    /// <summary>Löscht Profil, Passwort und gemerkte Liste. Heruntergeladene Mods bleiben auf der Festplatte.</summary>
    public async Task RemoveSelectedProfileAsync()
    {
        if (SelectedProfile is not { } profile || IsBusy)
        {
            return;
        }

        if (profile.Saved?.ServerAddress is { } address)
        {
            credentialStore.DeletePassword(address, profile.Id);
        }

        cache.Delete(profile.Id);
        var index = Profiles.IndexOf(profile);
        Profiles.Remove(profile);
        if (Profiles.Count == 0)
        {
            Profiles.Add(new ProfileViewModel());
        }

        SelectedProfile = Profiles[Math.Min(index, Profiles.Count - 1)];
        await PersistAsync();
        Log.Information("Server entfernt: {Name}", profile.DisplayName);
    }

    /// <summary>
    /// Holt zur Prüfung die Liste vom Server, dafür läuft kurz der ausgeblendete Spielclient. Ein Passwort
    /// wird nur gespeichert, wenn der Server es akzeptiert hat, alle anderen Werte in jedem Fall.
    /// </summary>
    /// <param name="keepPasswordForNewAddress">Der Nutzer hat zugestimmt, das bisherige Passwort für die neue Adresse zu verwenden.</param>
    /// <returns><c>false</c>, wenn das eingegebene Passwort nicht gespeichert wurde und im Eingabefeld bleiben soll.</returns>
    public async Task<bool> SaveSelectedProfileAsync(string enteredPassword, bool keepPasswordForNewAddress = false)
    {
        if (SelectedProfile is not { } profile || IsBusy)
        {
            return false;
        }

        if (await TryApplyChangesAsync(profile, keepPasswordForNewAddress) is not { } saved)
        {
            return false;
        }

        ShowSettingsMessage(profile, "Server wird geprüft, das dauert einen Moment …", isError: false);
        var result = await RunExclusiveAsync(token => orchestrator.CheckAsync(
            _settings,
            saved,
            enteredPassword.Length > 0 ? enteredPassword : null,
            new Progress<SyncStatus>(status => OnRunStatus(profile, status)),
            token));

        if (result.Outcome != SyncOutcome.Checked)
        {
            Log.Warning("Prüfung von Server {Name} fehlgeschlagen: {Message}", saved.Name, result.Message);
            var savedHint = enteredPassword.Length > 0
                ? "Die Einstellungen sind gespeichert, das Passwort noch nicht. Mit „Nur speichern“ legst du es ungeprüft ab."
                : "Die Einstellungen sind gespeichert.";
            ShowSettingsMessage(profile, $"{result.Message} {savedHint}", isError: true);
            return false;
        }

        if (!TryStorePassword(profile, saved, enteredPassword))
        {
            return false;
        }

        ReadPasswordState(profile);
        ShowMods(profile, result.Mods);
        SetStandNow(profile);
        SetStatus(profile, "", isError: false);
        Log.Information("Server {Name} gespeichert, {Count} Mods", saved.Name, result.Mods.Count);
        ShowSettingsMessage(profile, $"Gespeichert. Der Server listet {result.Mods.Count} Mods.", isError: false);
        SelectedTab = 0;
        return true;
    }

    /// <summary>
    /// Speichert ohne Verbindung zum Server, zum Beispiel wenn er gerade offline ist. Auch ein eingegebenes
    /// Passwort wird ungeprüft abgelegt, ob es stimmt, zeigt sich erst beim nächsten Kontakt mit dem Server.
    /// </summary>
    /// <returns><c>false</c>, wenn das eingegebene Passwort nicht gespeichert wurde und im Eingabefeld bleiben soll.</returns>
    public async Task<bool> SaveSelectedProfileWithoutCheckAsync(string enteredPassword, bool keepPasswordForNewAddress = false)
    {
        if (SelectedProfile is not { } profile || IsBusy)
        {
            return false;
        }

        if (await TryApplyChangesAsync(profile, keepPasswordForNewAddress) is not { } saved
            || !TryStorePassword(profile, saved, enteredPassword))
        {
            return false;
        }

        ReadPasswordState(profile);
        Log.Information("Server {Name} ohne Prüfung gespeichert", saved.Name);
        ShowSettingsMessage(profile, "Gespeichert, aber nicht geprüft. Ob alles passt, zeigt „Neu prüfen“ in der Übersicht, sobald der Server läuft.", isError: false);
        return true;
    }

    /// <summary>Prüft die Eingaben, schreibt sie in die Einstellungen und zieht Passwort und Mod-Liste nach.</summary>
    /// <returns>Der gespeicherte Stand, oder <c>null</c>, wenn dem Nutzer ein Problem angezeigt wird.</returns>
    private async Task<ServerProfile?> TryApplyChangesAsync(ProfileViewModel profile, bool keepPasswordForNewAddress)
    {
        if (string.IsNullOrWhiteSpace(profile.Name))
        {
            ShowSettingsMessage(profile, "Bitte einen Namen für diesen Server eintragen.", isError: true);
            return null;
        }

        var saved = profile.ToProfile();
        if (!ClientArgsBuilder.IsValidServerAddress(saved.ServerAddress))
        {
            ShowSettingsMessage(profile, "Bitte die Server-Adresse als IP und Port eintragen, zum Beispiel 203.0.113.10:2001.", isError: true);
            return null;
        }

        var previous = profile.Saved;
        profile.Saved = saved;
        await PersistAsync();

        if (previous is null)
        {
            return saved;
        }

        if (previous.ServerAddress != saved.ServerAddress)
        {
            // Die gemerkte Liste gehört zum alten Server und würde in der Übersicht falsche Mods zeigen.
            ResetModList(profile);
            if (previous.ServerAddress is { } previousAddress
                && !TryMovePassword(profile, previousAddress, saved.ServerAddress!, keepPasswordForNewAddress))
            {
                return null;
            }
        }
        else if (previous.DownloadDir != saved.DownloadDir)
        {
            await ShowCachedListAsync(profile);
        }

        return saved;
    }

    private bool TryStorePassword(ProfileViewModel profile, ServerProfile saved, string enteredPassword)
    {
        if (enteredPassword.Length == 0)
        {
            return true;
        }

        try
        {
            credentialStore.SavePassword(saved.ServerAddress!, profile.Id, enteredPassword);
            return true;
        }
        catch (System.ComponentModel.Win32Exception ex)
        {
            ShowSettingsMessage(profile, $"Das Passwort konnte nicht gespeichert werden: {ex.Message}", isError: true);
            return false;
        }
    }

    private void ResetModList(ProfileViewModel profile)
    {
        cache.Delete(profile.Id);
        profile.Mods.Clear();
        profile.OpenModCount = 0;
        profile.ProgressDone = 0;
        profile.ProgressTotal = 1;
        profile.ListStatus = "noch nicht geprüft";
        profile.Summary = "Noch nicht geprüft";
        profile.ListStand = "";
    }

    /// <summary>
    /// Die Adresse wurde geändert, ohne ein neues Passwort einzugeben. Ein Passwort gilt nur für seine Adresse:
    /// Für eine andere darf es nur mit Zustimmung des Nutzers verwendet werden, sonst ginge es unbemerkt an einen fremden Server.
    /// </summary>
    public bool NeedsPasswordTransferDecision(string enteredPassword) =>
        enteredPassword.Length == 0
        && SelectedProfile is { HasSavedPassword: true, Saved.ServerAddress: { } previous } profile
        && !string.IsNullOrWhiteSpace(profile.Name)
        && profile.ToProfile().ServerAddress is { } current
        && current != previous
        && ClientArgsBuilder.IsValidServerAddress(current);

    /// <summary>Der Eintrag der alten Adresse verschwindet in jedem Fall, damit kein verwaistes Passwort zurückbleibt.</summary>
    private bool TryMovePassword(ProfileViewModel profile, string previousAddress, string newAddress, bool keep)
    {
        try
        {
            if (keep && credentialStore.ReadPassword(previousAddress, profile.Id) is { } password)
            {
                credentialStore.SavePassword(newAddress, profile.Id, password);
            }

            credentialStore.DeletePassword(previousAddress, profile.Id);
        }
        catch (System.ComponentModel.Win32Exception ex)
        {
            ShowSettingsMessage(profile, $"Das Passwort konnte nicht übernommen werden: {ex.Message}", isError: true);
            return false;
        }
        finally
        {
            ReadPasswordState(profile);
        }

        return true;
    }

    public void DeleteSelectedPassword()
    {
        if (SelectedProfile is not { } profile)
        {
            return;
        }

        if (profile.Saved?.ServerAddress is { } address)
        {
            credentialStore.DeletePassword(address, profile.Id);
        }

        ReadPasswordState(profile);
        ShowSettingsMessage(profile, "Das gespeicherte Passwort dieses Servers wurde gelöscht.", isError: false);
    }

    [RelayCommand(CanExecute = nameof(CanRefresh))]
    private async Task RefreshAsync()
    {
        if (SelectedProfile is not { } profile)
        {
            return;
        }

        if (profile.Saved is not { } saved)
        {
            SetStatus(profile, "Bitte den Server zuerst in den Einstellungen speichern.", isError: true);
            return;
        }

        profile.ListStatus = "wird geprüft …";
        SetStatus(profile, "Mod-Liste wird vom Server geholt …", isError: false);
        var result = await RunExclusiveAsync(token => orchestrator.CheckAsync(
            _settings,
            saved,
            null,
            new Progress<SyncStatus>(status => OnRunStatus(profile, status)),
            token));

        if (result.Outcome == SyncOutcome.Checked)
        {
            ShowMods(profile, result.Mods);
            SetStandNow(profile);
            SetStatus(profile, "", isError: false);
        }
        else
        {
            profile.ListStatus = result.Outcome == SyncOutcome.Cancelled ? "abgebrochen" : "Fehler";
            SetStatus(profile, result.Message, isError: result.Outcome != SyncOutcome.Cancelled);
        }
    }

    private bool CanRefresh() => !IsBusy && SelectedProfile is not null;

    // Während des Laufs dient derselbe Befehl zum Abbrechen und muss deshalb klickbar bleiben.
    [RelayCommand(AllowConcurrentExecutions = true)]
    private async Task SyncAsync()
    {
        if (IsBusy)
        {
            if (SelectedProfile is { } running)
            {
                SetStatus(running, "Wird abgebrochen, der Spielclient wird beendet …", isError: false);
            }

            _runCancel?.Cancel();
            return;
        }

        if (SelectedProfile is not { } profile)
        {
            return;
        }

        if (profile.Saved is not { } saved || profile.ToProfile() != saved)
        {
            SetStatus(profile, "Dieser Server hat ungespeicherte Änderungen. Bitte zuerst in den Einstellungen speichern.", isError: true);
            return;
        }

        profile.LogText = "";
        profile.ListStatus = "synchronisiert …";
        SetStatus(profile, "Wird gestartet …", isError: false);
        var result = await RunExclusiveAsync(token => orchestrator.RunAsync(
            _settings,
            saved,
            new Progress<SyncStatus>(status => OnRunStatus(profile, status)),
            token));

        Log.Information("Sync {Name} beendet: {Outcome} - {Message}", saved.Name, result.Outcome, result.Message);
        AppendLog(profile, result.Message);
        SetStatus(profile, result.Message, isError: result.Outcome is SyncOutcome.Failed or SyncOutcome.Incomplete);
        if (result.Mods.Count > 0)
        {
            ShowMods(profile, result.Mods);
            SetStandNow(profile);
        }
        else
        {
            profile.ListStatus = result.Outcome == SyncOutcome.Failed ? "Fehler" : "abgebrochen";
        }

    }

    /// <summary>Startet das Spiel und tritt dem gewählten Server direkt bei.</summary>
    public async Task JoinServerAsync()
    {
        if (SelectedProfile is not { } profile || IsBusy)
        {
            return;
        }

        if (profile.Saved is not { } saved || profile.ToProfile() != saved)
        {
            SetStatus(profile, "Dieser Server hat ungespeicherte Änderungen. Bitte zuerst in den Einstellungen speichern.", isError: true);
            return;
        }

        if (!ClientArgsBuilder.IsValidServerAddress(saved.ServerAddress))
        {
            SetStatus(profile, "Es ist keine gültige Server-Adresse hinterlegt (IP:Port).", isError: true);
            return;
        }

        string? problem;
        var hasPassword = false;
        try
        {
            var password = credentialStore.ReadPassword(saved.ServerAddress!, profile.Id);
            hasPassword = !string.IsNullOrEmpty(password);
            problem = GameLauncher.StartAndJoin(_settings, saved, password);
        }
        catch (Exception ex) when (ex is System.IO.IOException or UnauthorizedAccessException or System.ComponentModel.Win32Exception)
        {
            problem = ex.Message;
        }

        if (problem is not null)
        {
            SetStatus(profile, problem, isError: true);
            return;
        }

        Log.Information("Direkter Beitritt gestartet für Server {Name}", saved.Name);
        SetStatus(profile, "Das Spiel startet und tritt dem Server bei. Klappt das nicht, bleibt es im Hauptmenü, und du kannst selbst beitreten.", isError: false);

        if (!hasPassword)
        {
            return;
        }

        // Die Hilfs-Mod löscht die Passwortdatei beim Start. Das ist die Absicherung, falls sie nicht dazu kommt.
        // Wird die App vorher geschlossen, übernimmt das App-Ende diese Aufgabe.
        _passwordCleanupDue = DateTime.UtcNow + PasswordCleanupDelay;
        await Task.Delay(PasswordCleanupDelay);
        GameLauncher.DeletePasswordFile();
    }

    public void RequestCancel() => _runCancel?.Cancel();

    private async Task<SyncResult> RunExclusiveAsync(Func<CancellationToken, Task<SyncResult>> run)
    {
        IsBusy = true;
        using var cancel = _runCancel = new CancellationTokenSource();
        try
        {
            return await run(cancel.Token);
        }
        finally
        {
            _runCancel = null;
            IsBusy = false;
        }
    }

    partial void OnSelectedProfileChanged(ProfileViewModel? value)
    {
        if (_initialized)
        {
            _ = PersistAsync();
        }
    }

    private void OnRunStatus(ProfileViewModel profile, SyncStatus status)
    {
        Log.Information("[{Name}] {Message}", profile.DisplayName, status.Message);
        AppendLog(profile, status.Message);
        if (!status.Message.StartsWith("[client]", StringComparison.Ordinal))
        {
            profile.StatusMessage = status.Message;
        }

        if (status.Mods is not null)
        {
            ShowMods(profile, status.Mods);
            profile.ListStatus = "synchronisiert …";
        }
    }

    private static void AppendLog(ProfileViewModel profile, string message) =>
        profile.LogText += $"{DateTime.Now:HH:mm:ss}  {message}\n";

    private async Task ShowCachedListAsync(ProfileViewModel profile)
    {
        if (await cache.LoadAsync(profile.Id) is not { } list)
        {
            return;
        }

        var installed = await AddonScanner.ScanAsync(AddonPaths.ResolveModsRoot(profile.Saved?.DownloadDir));
        ShowMods(profile, AddonComparer.Compare(list.Mods, installed));
        profile.ListStand = $"Liste vom {list.FetchedAt:g}";
    }

    private static void SetStandNow(ProfileViewModel profile) =>
        profile.ListStand = $"Liste vom {DateTime.Now:g}";

    private static void ShowMods(ProfileViewModel profile, IReadOnlyList<ModComparison> comparison)
    {
        profile.Mods.Clear();
        foreach (var mod in comparison.OrderBy(mod => mod.State == ModState.Current).ThenBy(mod => mod.Required.Name))
        {
            profile.Mods.Add(new ModRow(
                mod.Required.Name,
                mod.Required.Version ?? "beliebig",
                mod.Installed?.Version ?? "–",
                mod.State switch
                {
                    ModState.Current => "aktuell",
                    ModState.Missing => "fehlt",
                    ModState.Outdated => "veraltet",
                    ModState.Newer => "neuer als Server",
                    ModState.Different => "andere Version",
                    _ => "beschädigt",
                },
                mod.State));
        }

        var current = comparison.Count(mod => mod.State == ModState.Current);
        var open = comparison.Count - current;
        profile.OpenModCount = open;
        profile.ProgressTotal = Math.Max(comparison.Count, 1);
        profile.ProgressDone = current;
        profile.ListStatus = open == 0 ? "aktuell" : $"{open} von {comparison.Count} offen";
        profile.Summary = open == 0
            ? $"Alles aktuell: {comparison.Count} Mods"
            : string.Join(" · ", new[]
            {
                $"{current} von {comparison.Count} Mods aktuell",
                CountText(comparison, ModState.Missing, comparison.Count(mod => mod.State == ModState.Missing) == 1 ? "fehlt" : "fehlen"),
                CountText(comparison, ModState.Outdated, "veraltet"),
                CountText(comparison, ModState.Newer, "neuer als Server"),
                CountText(comparison, ModState.Different, "andere Version"),
                CountText(comparison, ModState.Damaged, "beschädigt"),
            }.Where(text => text.Length > 0));
    }

    private static string CountText(IReadOnlyList<ModComparison> comparison, ModState state, string label)
    {
        var count = comparison.Count(mod => mod.State == state);
        return count == 0 ? "" : $"{count} {label}";
    }

    private async Task PersistAsync()
    {
        await _saveGate.WaitAsync();
        try
        {
            _settings = _settings with
            {
                Profiles = Profiles.Where(p => p.Saved is not null).Select(p => p.Saved!).ToList(),
                SelectedProfileId = SelectedProfile?.Id,
            };
            await settingsStore.SaveAsync(_settings);
        }
        catch (Exception ex) when (ex is System.IO.IOException or UnauthorizedAccessException)
        {
            Log.Warning("Einstellungen konnten nicht gespeichert werden: {Message}", ex.Message);
            if (SelectedProfile is { } profile)
            {
                ShowSettingsMessage(profile, $"Die Einstellungen konnten nicht gespeichert werden: {ex.Message}", isError: true);
            }
        }
        finally
        {
            _saveGate.Release();
        }
    }

    private void ReadPasswordState(ProfileViewModel profile)
    {
        profile.HasSavedPassword = profile.Saved?.ServerAddress is { } address
            && credentialStore.ReadPassword(address, profile.Id) is not null;
        profile.IsEditingPassword = false;
    }

    private static bool IsConfigured(ProfileViewModel profile) =>
        profile.Saved is { } saved && ClientArgsBuilder.IsValidServerAddress(saved.ServerAddress);

    private static void ShowSettingsMessage(ProfileViewModel profile, string message, bool isError)
    {
        profile.SettingsMessage = message;
        profile.SettingsMessageIsError = isError;
    }

    private static void SetStatus(ProfileViewModel profile, string message, bool isError)
    {
        profile.StatusMessage = message;
        profile.StatusIsError = isError;
    }
}
