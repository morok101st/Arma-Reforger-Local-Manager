namespace ReforgerModSync.Core;

/// <summary>Angaben zum Produkt für den Bereich „Über“.</summary>
public static class AppInfo
{
    public const string RepositoryUrl = "https://github.com/morok101st/Arma-Reforger-Local-Manager";
    public const string ReleasesUrl = RepositoryUrl + "/releases";

    /// <summary>Entfernt den Commit, den der Build an die Version hängt (<c>0.3.1+2559257…</c>).</summary>
    public static string FormatVersion(string? informationalVersion)
    {
        var version = informationalVersion?.Split('+')[0].Trim();
        return string.IsNullOrEmpty(version) ? "unbekannt" : version;
    }
}
