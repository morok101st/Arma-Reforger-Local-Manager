namespace ReforgerModSync.Core.Addons;

public enum ModState
{
    Current,
    Missing,

    /// <summary>Lokal älter als vom Server verlangt.</summary>
    Outdated,

    /// <summary>Lokal neuer als vom Server verlangt, z. B. weil der Server noch nicht aktualisiert wurde.</summary>
    Newer,

    /// <summary>Lokal eine andere Version, die sich nicht einordnen lässt.</summary>
    Different,

    Damaged,
}

public sealed record ModComparison(ModEntry Required, ModState State, InstalledAddon? Installed);

public static class AddonComparer
{
    public static IReadOnlyList<ModComparison> Compare(
        IEnumerable<ModEntry> required,
        IEnumerable<InstalledAddon> installed)
    {
        // Zwei Ordner mit derselben ID sind möglich (umbenannte Mod), daher die gesunde Variante bevorzugen.
        var byId = installed
            .GroupBy(addon => addon.Id, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(
                group => group.Key,
                group => group.OrderBy(addon => addon.Health != AddonHealth.Ok).First(),
                StringComparer.OrdinalIgnoreCase);
        return required
            .Select(mod =>
            {
                var addon = byId.GetValueOrDefault(mod.Id);
                return new ModComparison(mod, GetState(mod, addon), addon);
            })
            .ToList();
    }

    private static ModState GetState(ModEntry mod, InstalledAddon? addon)
    {
        if (addon is null)
        {
            return ModState.Missing;
        }

        if (addon.Health != AddonHealth.Ok)
        {
            return ModState.Damaged;
        }

        if (mod.Version is null || mod.Version == addon.Version)
        {
            return ModState.Current;
        }

        return CompareVersions(addon.Version, mod.Version) switch
        {
            < 0 => ModState.Outdated,
            > 0 => ModState.Newer,
            0 => ModState.Current,
            null => ModState.Different,
        };
    }

    /// <summary>Vergleicht Versionen wie <c>0.16.5236</c> Teil für Teil. <c>null</c>: nicht vergleichbar.</summary>
    public static int? CompareVersions(string? left, string? right)
    {
        if (ParseVersion(left) is not { } leftParts || ParseVersion(right) is not { } rightParts)
        {
            return null;
        }

        for (var i = 0; i < Math.Max(leftParts.Length, rightParts.Length); i++)
        {
            var difference = leftParts.ElementAtOrDefault(i).CompareTo(rightParts.ElementAtOrDefault(i));
            if (difference != 0)
            {
                return difference;
            }
        }

        return 0;
    }

    private static long[]? ParseVersion(string? version)
    {
        if (string.IsNullOrWhiteSpace(version))
        {
            return null;
        }

        var parts = version.Split('.');
        var numbers = new long[parts.Length];
        for (var i = 0; i < parts.Length; i++)
        {
            if (!long.TryParse(parts[i], out numbers[i]))
            {
                return null;
            }
        }

        return numbers;
    }
}
