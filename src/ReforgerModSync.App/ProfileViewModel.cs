using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;
using ReforgerModSync.Core.Settings;

namespace ReforgerModSync.App;

public sealed record ModRow(string Name, string Required, string Local, string State, ModState Kind)
{
    /// <summary>Passt nicht zum Server: Der Status wird in der Liste hervorgehoben.</summary>
    public bool IsOpen => Kind != ModState.Current;

    public bool IsDamaged => Kind == ModState.Damaged;
}

/// <summary>Ein Server mit Eingabefeldern und eigenem Stand für die Übersicht.</summary>
public sealed partial class ProfileViewModel : ObservableObject
{
    public ProfileViewModel(ServerProfile? saved = null)
    {
        Id = saved?.Id ?? Guid.NewGuid().ToString("N");
        Saved = saved is null ? null : saved with { DownloadDir = AddonPaths.NormalizeDownloadDir(saved.DownloadDir) };
        _name = saved?.Name ?? "";
        _serverAddress = saved?.ServerAddress ?? "";
        _downloadDir = Saved?.DownloadDir ?? "";
    }

    public string Id { get; }

    /// <summary>Der zuletzt gespeicherte Stand. Ein Sync nutzt nur diesen. <c>null</c>: noch nie gespeichert.</summary>
    public ServerProfile? Saved { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(DisplayName))]
    private string _name;

    [ObservableProperty] private string _serverAddress;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(SteamLaunchOptions), nameof(HasDownloadDir), nameof(DownloadDirDisplay))]
    private string _downloadDir;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowSavedPassword), nameof(ShowPasswordInput), nameof(CanCancelPasswordEdit), nameof(PasswordHint))]
    private bool _hasSavedPassword;

    /// <summary>Der Nutzer hat bei einem gespeicherten Passwort „Ändern“ gewählt.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowSavedPassword), nameof(ShowPasswordInput), nameof(CanCancelPasswordEdit), nameof(PasswordHint))]
    private bool _isEditingPassword;

    public bool ShowSavedPassword => HasSavedPassword && !IsEditingPassword;
    public bool ShowPasswordInput => !ShowSavedPassword;
    public bool CanCancelPasswordEdit => HasSavedPassword && IsEditingPassword;

    public string PasswordHint => (HasSavedPassword, IsEditingPassword) switch
    {
        (true, false) => "Sicher im Windows-Anmeldeinformationsspeicher abgelegt.",
        (true, true) => "Neues Passwort eingeben und unten speichern. Es ersetzt das bisherige.",
        _ => "Leer lassen, wenn der Server kein Passwort hat.",
    };

    [ObservableProperty] private string _settingsMessage = "";
    [ObservableProperty] private bool _settingsMessageIsError;

    [ObservableProperty] private string _listStatus = "noch nicht geprüft";
    [ObservableProperty] private string _summary = "Noch nicht geprüft";
    [ObservableProperty] private string _listStand = "";
    [ObservableProperty] private string _statusMessage = "";
    [ObservableProperty] private bool _statusIsError;
    [ObservableProperty] private int _progressDone;
    [ObservableProperty] private int _progressTotal = 1;
    [ObservableProperty] private string _logText = "";

    public ObservableCollection<ModRow> Mods { get; } = [];

    /// <summary>Mods, die laut letztem Abgleich nicht zum Server passen.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasOpenMods))]
    private int _openModCount;

    public bool HasOpenMods => OpenModCount > 0;

    public string DisplayName => string.IsNullOrWhiteSpace(Name) ? "Neuer Server" : Name;
    public bool HasDownloadDir => !string.IsNullOrWhiteSpace(DownloadDir);
    public string DownloadDirDisplay => HasDownloadDir ? DownloadDir : "Standardordner des Spiels";
    public string SteamLaunchOptions => ClientArgsBuilder.SteamLaunchOptions(DownloadDir);

    public ServerProfile ToProfile() => new()
    {
        Id = Id,
        Name = Name.Trim(),
        ServerAddress = string.IsNullOrWhiteSpace(ServerAddress) ? null : ServerAddress.Trim(),
        DownloadDir = AddonPaths.NormalizeDownloadDir(DownloadDir),
    };
}
