using ReforgerModSync.Core.Addons;
using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Sync;

/// <summary>Ein Listenabruf ist gescheitert. Die Meldung ist für den Nutzer gedacht.</summary>
public sealed class ServerListException(string message) : Exception(message);

/// <summary>Liest die Mod-Liste, die die Hilfs-Mod mit <c>[RMS] SERVER_MOD</c> ins Log des Clients schreibt.</summary>
public sealed class ServerListReader(TimeSpan pollInterval, TimeSpan timeout)
{
    public static ServerListReader Default { get; } = new(TimeSpan.FromMilliseconds(500), TimeSpan.FromMinutes(2));

    /// <exception cref="ServerListException">Kein vollständiges Ergebnis, mit Meldung für den Nutzer.</exception>
    public async Task<IReadOnlyList<ModEntry>> ReadAsync(
        IClientProcess client,
        string logsDir,
        string address,
        CancellationToken cancellationToken)
    {
        var mods = new List<ModEntry>();
        var tail = new LogTail(Path.Combine(logsDir, "console.log"));
        var deadline = DateTime.UtcNow + timeout;
        while (DateTime.UtcNow < deadline)
        {
            await Task.Delay(pollInterval, cancellationToken);

            // Erst den Prozessstand lesen: Beendet er sich gerade, stehen seine letzten Zeilen schon im Log.
            var exited = client.HasExited;
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

        throw new ServerListException($"Die Mod-Liste kam nicht innerhalb von {timeout.TotalMinutes:0} Minuten an. Client-Logs: {logsDir}");
    }
}
