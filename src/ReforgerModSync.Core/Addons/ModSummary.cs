namespace ReforgerModSync.Core.Addons;

/// <summary>Stand eines Abgleichs in Zahlen und als Text für Übersicht und Server-Liste.</summary>
/// <param name="ListStatus">Kurz für die Server-Liste, z. B. „3 von 10 offen“.</param>
/// <param name="Text">Ausführlich für die Übersicht, z. B. „7 von 10 Mods aktuell · 2 fehlen · 1 veraltet“.</param>
public sealed record ModSummary(int Total, int Current, string ListStatus, string Text)
{
    public int Open => Total - Current;

    public static ModSummary From(IReadOnlyList<ModComparison> comparison)
    {
        var current = comparison.Count(mod => mod.State == ModState.Current);
        var open = comparison.Count - current;
        if (open == 0)
        {
            return new ModSummary(comparison.Count, current, "aktuell", $"Alles aktuell: {comparison.Count} Mods");
        }

        var missing = comparison.Count(mod => mod.State == ModState.Missing);
        var parts = new[]
        {
            $"{current} von {comparison.Count} Mods aktuell",
            Count(comparison, ModState.Missing, missing == 1 ? "fehlt" : "fehlen"),
            Count(comparison, ModState.Outdated, "veraltet"),
            Count(comparison, ModState.Newer, "neuer als Server"),
            Count(comparison, ModState.Different, "andere Version"),
            Count(comparison, ModState.Damaged, "beschädigt"),
        };
        return new ModSummary(
            comparison.Count,
            current,
            $"{open} von {comparison.Count} offen",
            string.Join(" · ", parts.Where(part => part.Length > 0)));
    }

    /// <summary>Offene Mods zuerst, dann nach Name.</summary>
    public static IEnumerable<ModComparison> Sort(IEnumerable<ModComparison> comparison) =>
        comparison.OrderBy(mod => mod.State == ModState.Current).ThenBy(mod => mod.Required.Name);

    public static string StateText(ModState state) => state switch
    {
        ModState.Current => "aktuell",
        ModState.Missing => "fehlt",
        ModState.Outdated => "veraltet",
        ModState.Newer => "neuer als Server",
        ModState.Different => "andere Version",
        _ => "beschädigt",
    };

    private static string Count(IReadOnlyList<ModComparison> comparison, ModState state, string label)
    {
        var count = comparison.Count(mod => mod.State == state);
        return count == 0 ? "" : $"{count} {label}";
    }
}
