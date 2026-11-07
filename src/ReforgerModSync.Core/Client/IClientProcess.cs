namespace ReforgerModSync.Core.Client;

/// <summary>Ein laufender Spielclient, so wie Liste und Download ihn brauchen. Tests setzen einen eigenen ein.</summary>
public interface IClientProcess : IDisposable
{
    bool HasExited { get; }

    /// <summary>Erst sauber schließen, sonst beenden.</summary>
    Task StopAsync();
}

/// <summary>Startet den Spielclient mit diesen Startparametern.</summary>
/// <param name="hideWindow">Spielfenster ausblenden.</param>
public delegate IClientProcess ClientStarter(string exe, IReadOnlyList<string> arguments, bool hideWindow);
