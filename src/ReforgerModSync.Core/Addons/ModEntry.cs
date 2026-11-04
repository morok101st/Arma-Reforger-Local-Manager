namespace ReforgerModSync.Core.Addons;

/// <summary>Eine Mod aus der Liste des Servers. Ohne Version zählt jede vollständige lokale Version als aktuell.</summary>
public sealed record ModEntry(string Id, string Name, string? Version = null);
