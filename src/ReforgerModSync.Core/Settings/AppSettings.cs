namespace ReforgerModSync.Core.Settings;

/// <summary>Einstellungen pro Windows-Benutzer. Enthält nie Geheimnisse.</summary>
public sealed record AppSettings
{
    public IReadOnlyList<ServerProfile> Profiles { get; init; } = [];
    public string? SelectedProfileId { get; init; }

    /// <summary><c>null</c>: Client automatisch über Steam erkennen.</summary>
    public string? ReforgerExe { get; init; }

    public ClientWindowSettings Client { get; init; } = new();
    public TimeoutSettings Timeouts { get; init; } = new();

    /// <summary>Zielordner, in denen der Sync überzählige Mods löschen darf. Der Nutzer stimmt je Ordner einmal zu.</summary>
    public IReadOnlyList<string> CleanupApprovedFolders { get; init; } = [];
}

/// <summary>Ein Spielserver mit eigenem Zielordner. Sein Passwort liegt im Anmeldeinformationsspeicher.</summary>
public sealed record ServerProfile
{
    public required string Id { get; init; }
    public required string Name { get; init; }

    /// <summary>IP und Port, z. B. <c>203.0.113.10:2001</c>. Die Mod-Liste kommt von diesem Server.</summary>
    public string? ServerAddress { get; init; }

    /// <summary><c>null</c>: Standardordner des Clients nutzen.</summary>
    public string? DownloadDir { get; init; }
}

public sealed record ClientWindowSettings
{
    public int Width { get; init; } = 640;
    public int Height { get; init; } = 360;
    public int MaxFps { get; init; } = 15;

    /// <summary>Spielfenster beim Sync ausblenden. <c>false</c> hilft bei der Fehlersuche.</summary>
    public bool HideWindow { get; init; } = true;
}

public sealed record TimeoutSettings
{
    public int TotalMinutes { get; init; } = 120;
    public int IdleMinutes { get; init; } = 10;
}
