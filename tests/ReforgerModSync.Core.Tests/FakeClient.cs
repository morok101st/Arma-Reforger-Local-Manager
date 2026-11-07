using ReforgerModSync.Core.Client;

namespace ReforgerModSync.Core.Tests;

/// <summary>Steht in Tests für den Spielclient. Schreibt auf Wunsch ein Beispiel-Log wie der echte Client.</summary>
internal sealed class FakeClient : IClientProcess
{
    public bool HasExited { get; set; }

    public bool Stopped { get; private set; }

    public bool Disposed { get; private set; }

    public Task StopAsync()
    {
        Stopped = true;
        HasExited = true;
        return Task.CompletedTask;
    }

    public void Dispose() => Disposed = true;

    public static string Sample(string name) =>
        File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Samples", name));

    /// <summary>Legt <c>console.log</c> an, wie der Client es im Log-Ordner tut.</summary>
    public static void WriteLog(string logsDir, string content) =>
        File.WriteAllText(Path.Combine(logsDir, "console.log"), content);
}
