namespace ReforgerModSync.Core;

/// <summary>Ablageorte der App unter <c>%LOCALAPPDATA%\ReforgerModSync</c>. Tests übergeben einen eigenen Basisordner.</summary>
public sealed record AppPaths(string LocalData)
{
    public static AppPaths Default { get; } = new(Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "ReforgerModSync"));

    /// <summary>Je Lauf ein Ordner für die Logs des Spielclients.</summary>
    public string ClientLogs => Path.Combine(LocalData, "clientlogs");

    /// <summary>Eigenes Profil des Spielclients für Liste und Download, die Einstellungen des Spielers bleiben unberührt.</summary>
    public string ClientProfile => Path.Combine(LocalData, "clientprofile");

    public string ModLists => Path.Combine(LocalData, "modlists");

    public string Logs => Path.Combine(LocalData, "logs");
}
