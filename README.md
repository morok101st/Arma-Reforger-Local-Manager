# Reforger Sync & Launch

**Sync & Launch** lädt die Mods eines Arma-Reforger-Servers vorab herunter, damit du beim Beitritt nicht mehr warten musst.

![Übersicht mit Mod-Liste und Status](.github/screenshots/uebersicht.png)

## Wofür ist das?

Beim Beitritt zu einem Server lädt Arma Reforger fehlende Mods erst dann, und du wartest im Ladebildschirm. Mit Sync & Launch erledigst du das vorher, zum Beispiel am Abend vor einem Event. Beim Beitritt ist dann schon alles da.

## Was macht es?

- **Mods abgleichen:** Holt die Mod-Liste direkt vom Spielserver und zeigt, welche Mods fehlen, veraltet oder aktuell sind.
- **Mods laden:** Bringt alle Mods auf die Version des Servers. Das Spielfenster bleibt dabei ausgeblendet.
- **Direkt beitreten:** Startet das Spiel und verbindet dich mit dem Server.
- **Mehrere Server verwalten:** Jeder Server hat seine eigene Adresse, sein Passwort und seinen Zielordner. Weitere Server legst du links mit **Server hinzufügen** an.

Den Download übernimmt dein installierter Arma-Reforger-Client mit einer kleinen Hilfs-Mod, die dem Paket beiliegt. Das Tool spricht nicht selbst mit dem Bohemia Workshop.

## Voraussetzungen

- Windows 10 oder 11, 64 Bit
- Steam läuft, Arma Reforger ist installiert
- Adresse (IP:Port) und gegebenenfalls Passwort des Servers
- Zum Prüfen und Synchronisieren muss der Server laufen, denn von ihm kommt die Mod-Liste

## Installation

1. Die ZIP-Datei von der Seite [Releases](../../releases) herunterladen.
2. In einen festen Ordner entpacken, zum Beispiel `C:\Spiele\ReforgerSyncLaunch`. Der Ordner `helper` muss neben der EXE bleiben.
3. `ReforgerSyncLaunch.exe` starten.

Die Testversion ist nicht signiert, deshalb warnt Windows eventuell beim ersten Start. Klicke dann auf „Weitere Informationen“ und „Trotzdem ausführen“.

**Aktualisieren:** Deine Version steht in der Titelleiste und links unten in der App, der Link **Alle Versionen** führt zu den Releases. Schließe die App und entpacke die neue ZIP-Datei in denselben Ordner. Einstellungen, Passwörter und Mods bleiben erhalten.

## Was muss ich einstellen?

![Reiter Einstellungen](.github/screenshots/einstellungen.png)

Alles wird im Reiter **Einstellungen** eingetragen:

| Feld | Bedeutung |
|---|---|
| **Name** | Frei wählbar, zum Beispiel „Training“ |
| **Server-Adresse** | IP und Port des Spielservers, zum Beispiel `203.0.113.10:2001` |
| **Server-Passwort** | Leer lassen, wenn der Server keines hat. Ein gespeichertes Passwort erscheint als Punkte mit „✓ Gespeichert“ und lässt sich über **Ändern** und **Löschen** bearbeiten |
| **Zielordner** | Wohin die Mods geladen werden. Leer lassen, um den Standardordner des Spiels zu nutzen |

Klicke danach auf **Speichern**. Dabei wird noch nicht geprüft, ob Adresse und Passwort stimmen, der Server darf also gerade offline sein. Das zeigt sich erst in der **Übersicht** mit **Neu prüfen** oder **Jetzt synchronisieren**. Beides braucht einen laufenden Server.

**Zielordner:** Die Mods liegen darin im Unterordner `addons`. Wähle den Ordner, der deinen vorhandenen `addons`-Ordner enthält, sonst wird alles neu geladen.

**Start über Steam (optional):** Mit „Direkt beitreten“ musst du nichts weiter einstellen, das Spiel nutzt deinen Zielordner automatisch. Nur wenn du das Spiel direkt über Steam startest und einen eigenen Zielordner gewählt hast, klappe „Nur beim Start über Steam“ auf und kopiere den Text in die Startoptionen (Arma Reforger › Eigenschaften › Startoptionen). Sonst lädt das Spiel beim Beitritt alles erneut.

## Benutzung

1. Das Spiel schließen.
2. Optional in der **Übersicht** auf **Neu prüfen** klicken. Das holt nur die Mod-Liste vom Server und zeigt, was fehlt, ohne etwas herunterzuladen.
3. Auf **Jetzt synchronisieren** klicken. Das erste Mal kann das lange dauern. Während des Laufs wird daraus **Abbrechen**, bereits geladene Mods bleiben dabei erhalten.
4. Auf **Direkt beitreten** klicken, sobald alles aktuell ist.

Beim Schließen des Fensters wird die App beendet. Ein laufender Sync bricht dabei ab, vorher fragt die App nach. Das Spiel läuft nach „Direkt beitreten“ auch ohne die App weiter.

## Gut zu wissen

- **Versionen:** Ein Sync bringt Mods auf die Version des Servers, auch zurück auf eine ältere. Das gilt für den gewählten Zielordner. Wer auf anderen Servern andere Versionen braucht, nimmt einen eigenen Zielordner.
- **Passwort:** Es liegt verschlüsselt im Windows-Anmeldeinformationsspeicher. Für den Abruf beim Server steht es kurz in einer temporären Datei, die danach sofort gelöscht wird. Änderst du die Server-Adresse, fragt die App, ob das Passwort auch für die neue Adresse gelten soll.
- **BattlEye:** Beim Spielstart kann Windows nach Administratorrechten fragen.
- **Lizenzen:** Die Mods stammen aus dem Arma Reforger Workshop. Für jede Mod gelten ihre Lizenz und die [Workshop-Bedingungen](https://reforger.armaplatform.com/workshop-terms) von Bohemia Interactive.

Dies ist kein offizielles Produkt von Bohemia Interactive.

## Probleme melden

Nenne deine Version (steht in der Titelleiste und links unten in der App), beschreibe, was du geklickt hast und was passiert ist, und schicke diese beiden Ordner mit. Darin steht kein Passwort, aber die Server-Adresse und dein Windows-Benutzername:

```
%LOCALAPPDATA%\ReforgerModSync\logs
%LOCALAPPDATA%\ReforgerModSync\clientlogs
```

## Entfernen

1. In der App das Passwort über **Server entfernen** löschen.
2. Den Ordner der App löschen.
3. Optional auch `%APPDATA%\ReforgerModSync` und `%LOCALAPPDATA%\ReforgerModSync` löschen.

Die geladenen Mods bleiben erhalten.

## Entwicklung

Gebaut mit C#, .NET 10 und WPF. Benötigt wird das .NET 10 SDK unter Windows.

```powershell
dotnet test ReforgerModSync.slnx                                                   # Tests
powershell -ExecutionPolicy Bypass -File scripts\build-package.ps1 -Version 0.3.2  # Paket bauen
```

Ein Tag wie `v0.3.2` baut das Paket über GitHub Actions und veröffentlicht es als Release. Die App zeigt dann genau diese Version an, lokale Builds melden `0.0.0-dev`. Versionen unter 1.0 erscheinen als Vorabversion.
