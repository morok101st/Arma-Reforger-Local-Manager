using System.Text;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Credentials;
using ReforgerModSync.Core.Settings;
using ReforgerModSync.Core.Steam;
using ReforgerModSync.Core.Sync;

Console.OutputEncoding = Encoding.UTF8;

if (args is not (["status"] or ["sync"] or ["sync", "--server", _]))
{
    Console.WriteLine("Verwendung: ReforgerModSync.Cli status | sync [--server <Name>]");
    Console.WriteLine("Server, Passwörter und Zielordner werden in der App hinterlegt.");
    return args.Length == 0 ? 0 : 1;
}

try
{
    var settings = await new SettingsStore(SettingsStore.DefaultFilePath).LoadAsync();
    var credentialStore = new WindowsCredentialStore();
    var cache = new ModListCache(ModListCache.DefaultFolder);
    return args[0] == "sync"
        ? await SyncAsync(settings, credentialStore, cache, args.Length == 3 ? args[2] : null)
        : await StatusAsync(settings, credentialStore, cache);
}
catch (Exception ex) when (ex is SettingsException or ArgumentException or IOException or UnauthorizedAccessException)
{
    Console.Error.WriteLine($"Fehler: {ex.Message}");
    return 1;
}

static async Task<int> SyncAsync(AppSettings settings, ICredentialStore credentialStore, ModListCache cache, string? serverName)
{
    var profile = serverName is null
        ? settings.Profiles.FirstOrDefault(p => p.Id == settings.SelectedProfileId) ?? settings.Profiles.FirstOrDefault()
        : settings.Profiles.FirstOrDefault(p => string.Equals(p.Name, serverName, StringComparison.OrdinalIgnoreCase));
    if (profile is null)
    {
        Console.Error.WriteLine(serverName is null ? "Es ist noch kein Server angelegt." : $"Server „{serverName}“ nicht gefunden.");
        return 1;
    }

    using var cancel = new CancellationTokenSource();
    Console.CancelKeyPress += (_, e) =>
    {
        e.Cancel = true;
        cancel.Cancel();
    };

    Console.WriteLine($"Server: {profile.Name}");
    var progress = new Progress<SyncStatus>(status => Console.WriteLine(status.Message));
    var result = await new SyncOrchestrator(credentialStore, cache).RunAsync(settings, profile, progress, cancel.Token);
    Console.WriteLine(result.Message);
    return result.Outcome is SyncOutcome.AlreadyCurrent or SyncOutcome.Completed ? 0 : 1;
}

/// <summary>Startet keinen Client: zeigt den Vergleich mit der zuletzt vom Server geholten Liste.</summary>
static async Task<int> StatusAsync(AppSettings settings, ICredentialStore credentialStore, ModListCache cache)
{
    var steamPath = SteamLocator.ReadSteamPathFromRegistry();
    var clientExe = SteamLocator.ResolveClientExe(settings.ReforgerExe);
    Console.WriteLine($"Steam:  {steamPath ?? "nicht gefunden"} ({(SteamLocator.IsSteamRunning() ? "läuft" : "läuft nicht")})");
    Console.WriteLine($"Client: {clientExe ?? "nicht gefunden"} ({(SteamLocator.IsGameRunning() ? "läuft" : "läuft nicht")})");
    Console.WriteLine($"Server: {settings.Profiles.Count}");

    foreach (var profile in settings.Profiles)
    {
        var hasPassword = new ProfilePasswords(credentialStore).HasPassword(profile.ServerAddress, profile.Id);
        var modsRoot = AddonPaths.ResolveModsRoot(profile.DownloadDir);
        var addons = await AddonScanner.ScanAsync(modsRoot);
        Console.WriteLine();
        Console.WriteLine($"[{profile.Name}]");
        Console.WriteLine($"  Adresse:    {profile.ServerAddress ?? "nicht hinterlegt"}");
        Console.WriteLine($"  Passwort:   {(hasPassword ? "hinterlegt" : "nicht hinterlegt")}");
        Console.WriteLine($"  Mod-Ordner: {modsRoot} ({addons.Count} Mods)");

        if (await cache.LoadAsync(profile.Id) is not { } list)
        {
            Console.WriteLine("  Modset:     noch nicht vom Server geholt");
            continue;
        }

        var comparison = AddonComparer.Compare(list.Mods, addons);
        Console.WriteLine($"  Modset:     {ModSummary.From(comparison).Text} (Stand {list.FetchedAt:g})");
        foreach (var mod in ModSummary.Sort(comparison).Where(mod => mod.State != ModState.Current))
        {
            Console.WriteLine($"    [{ModSummary.StateText(mod.State)}] {mod.Required.Name}: benötigt {mod.Required.Version ?? "beliebig"}, lokal {mod.Installed?.Version ?? "-"}");
        }
    }

    return 0;
}
