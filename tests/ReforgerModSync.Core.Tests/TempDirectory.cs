namespace ReforgerModSync.Core.Tests;

internal sealed class TempDirectory : IDisposable
{
    public string Path { get; } = Directory.CreateTempSubdirectory("ReforgerModSyncTests").FullName;

    public void Dispose() => Directory.Delete(Path, recursive: true);
}
