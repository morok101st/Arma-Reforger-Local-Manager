// Oberfläche der App. Gespeicherte Einstellungen liegen im Backend, hier nur Entwürfe und die Anzeige.
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type Overview,
  type ProfileInput,
  type ProfileView,
  type RunResult,
  type RunStatus,
  type UpdateProgress,
  type UpdateView,
} from "./api";
import { basicSettingsDialog, confirmDialog, informDialog } from "./dialogs";

interface Draft {
  name: string;
  ip: string;
  port: string;
  downloadDir: string;
}

/** Ein Server mit Entwurf der Eingaben und eigenem Stand für die Übersicht. */
interface Profile {
  id: string;
  saved: ProfileView | null;
  draft: Draft;
  hasPassword: boolean;
  editingPassword: boolean;
  overview: Overview | null;
  listStatus: string;
  summary: string;
  stand: string;
  progressDone: number;
  progressTotal: number;
  status: string;
  statusIsError: boolean;
  settingsMessage: string;
  settingsIsError: boolean;
  log: string;
}

const state = {
  profiles: [] as Profile[],
  selectedId: null as string | null,
  tab: 0,
  busy: false,
  /** Während des Bereinigens gibt es nichts mehr abzubrechen. */
  cancellable: false,
  /** Nach „Direkt beitreten“: Das Spiel startet oder läuft. Solange sind Beitritt und Sync gesperrt. */
  game: "idle" as "idle" | "starting" | "running",
  /** Name des Servers, dem gerade beigetreten wird. */
  joinName: "",
};

const $ = <T extends HTMLElement = HTMLElement>(selector: string) => document.querySelector<T>(selector)!;

const ui = {
  serverList: $<HTMLUListElement>("#server-list"),
  version: $("#version"),
  tabs: [...document.querySelectorAll<HTMLButtonElement>(".tab")],
  overview: $("#overview"),
  settings: $("#settings"),
  title: $("#ov-title"),
  hint: $("#ov-hint"),
  summary: $("#ov-summary"),
  sync: $<HTMLButtonElement>("#sync"),
  refresh: $<HTMLButtonElement>("#refresh"),
  join: $<HTMLButtonElement>("#join"),
  joinNotice: $("#join-notice"),
  joinNoticeTitle: $("#join-notice-title"),
  joinNoticeText: $("#join-notice-text"),
  progress: $("#ov-progress"),
  status: $("#ov-status"),
  rows: $<HTMLTableSectionElement>("#ov-rows"),
  log: $<HTMLPreElement>("#ov-log"),
  form: $<HTMLFormElement>("#settings-form"),
  name: $<HTMLInputElement>("#f-name"),
  ip: $<HTMLInputElement>("#f-ip"),
  port: $<HTMLInputElement>("#f-port"),
  password: $<HTMLInputElement>("#f-password"),
  pwSaved: $("#pw-saved"),
  pwEdit: $("#pw-edit"),
  pwCancel: $<HTMLButtonElement>("#pw-cancel"),
  pwHint: $("#pw-hint"),
  dir: $<HTMLInputElement>("#f-dir"),
  dirExtra: $("#dir-extra"),
  steamOptions: $<HTMLInputElement>("#steam-options"),
  save: $<HTMLButtonElement>("#save"),
  settingsMessage: $("#settings-message"),
  armmSection: $("#armm-section"),
  armmTarget: $("#armm-target"),
  armmKey: $("#armm-key"),
  addressHint: $("#address-hint"),
};

/** Wie `normalize_address_input` im Backend: Schema, Pfad und Punkt am Ende weg, Kleinbuchstaben. */
function normalizeAddress(input: string): string {
  let text = input.trim();
  const scheme = text.indexOf("://");
  if (scheme >= 0) {
    text = text.slice(scheme + 3);
  }
  text = text.split(/[/?#]/)[0] ?? "";
  return text.replace(/\.+$/, "").toLowerCase();
}

function splitAddress(address: string | null): [string, string] {
  const text = normalizeAddress(address ?? "");
  const separator = text.lastIndexOf(":");
  return separator < 0 ? [text, ""] : [text.slice(0, separator).trim(), text.slice(separator + 1).trim()];
}

function draftOf(saved: ProfileView | null): Draft {
  const [ip, port] = splitAddress(saved?.serverAddress ?? null);
  return { name: saved?.name ?? "", ip, port, downloadDir: saved?.downloadDir ?? "" };
}

function newProfile(saved: ProfileView | null): Profile {
  const profile: Profile = {
    id: saved?.id ?? crypto.randomUUID().replaceAll("-", ""),
    saved,
    draft: draftOf(saved),
    hasPassword: saved?.hasPassword ?? false,
    editingPassword: false,
    overview: null,
    listStatus: "noch nicht geprüft",
    summary: "Noch nicht geprüft",
    stand: "",
    progressDone: 0,
    progressTotal: 1,
    status: "",
    statusIsError: false,
    settingsMessage: "",
    settingsIsError: false,
    log: "",
  };
  if (saved?.overview) {
    showOverview(profile, saved.overview);
  }
  return profile;
}

function showOverview(profile: Profile, overview: Overview) {
  profile.overview = overview;
  profile.listStatus = overview.summary.listStatus;
  profile.summary = overview.summary.text;
  profile.progressDone = overview.summary.current;
  profile.progressTotal = Math.max(overview.summary.total, 1);
  if (overview.stand) {
    profile.stand = overview.stand;
  }
}

function resetOverview(profile: Profile) {
  profile.overview = null;
  profile.listStatus = "noch nicht geprüft";
  profile.summary = "Noch nicht geprüft";
  profile.stand = "";
  profile.progressDone = 0;
  profile.progressTotal = 1;
}

const SERVER_ADDRESS_HINT =
  "Name oder IP-Adresse des Servers, zum Beispiel arma.example.de oder 203.0.113.10, dazu der Port, etwa 2001. Darüber wird der Server bei Bohemia gefunden, von dort kommt die Mod-Liste. Der Server muss dafür laufen.";
const ARMM_ADDRESS_HINT =
  "Name oder IP-Adresse des Servers, dazu der Port. Gebraucht für „Direkt beitreten“ und als Rückfall, falls eine Mod ohne Server nicht erreichbar ist. Die Mod-Liste kommt aus ARMM.";

const selected = () => state.profiles.find((profile) => profile.id === state.selectedId) ?? null;
const displayName = (profile: Profile) => profile.draft.name.trim() || "Neuer Server";
const toInput = (profile: Profile): ProfileInput => ({ id: profile.id, ...profile.draft });

/** Wie die Adresse gespeichert würde, damit ungespeicherte Änderungen erkennbar sind. */
function combinedAddress(draft: Draft): string | null {
  const ip = normalizeAddress(draft.ip);
  const port = draft.port.trim();
  if (!ip && !port) {
    return null;
  }
  return `${ip}:${/^\d+$/.test(port) ? String(Number(port)) : port}`;
}

function normalizeDir(dir: string): string | null {
  let path = dir.trim().replace(/[\\/]+$/, "");
  if (!path) {
    return null;
  }
  const parts = path.split(/[\\/]/);
  if (parts.length > 1 && parts.at(-1)!.toLowerCase() === "addons") {
    path = parts.slice(0, -1).join("\\").replace(/[\\/]+$/, "");
  }
  return path.endsWith(":") ? `${path}\\` : path;
}

function hasUnsavedChanges(profile: Profile): boolean {
  const saved = profile.saved;
  return (
    !saved ||
    saved.name !== profile.draft.name.trim() ||
    saved.serverAddress !== combinedAddress(profile.draft) ||
    (saved.downloadDir ?? null) !== normalizeDir(profile.draft.downloadDir)
  );
}

function appendLog(profile: Profile, message: string) {
  const time = new Date().toLocaleTimeString("de-DE");
  profile.log += `${time}  ${message}\n`;
}

function setStatus(profile: Profile, message: string, isError: boolean) {
  profile.status = message;
  profile.statusIsError = isError;
}

function setSettingsMessage(profile: Profile, message: string, isError: boolean) {
  profile.settingsMessage = message;
  profile.settingsIsError = isError;
}

// --- Darstellung ---

function renderList() {
  ui.serverList.replaceChildren(
    ...state.profiles.map((profile) => {
      const item = document.createElement("li");
      item.role = "option";
      item.tabIndex = 0;
      item.dataset.id = profile.id;
      item.ariaSelected = String(profile.id === state.selectedId);
      const name = Object.assign(document.createElement("div"), { className: "name", textContent: displayName(profile) });
      const status = Object.assign(document.createElement("div"), { className: "status", textContent: profile.listStatus });
      item.append(name, status);
      return item;
    }),
  );
}

function renderTabs() {
  ui.tabs.forEach((tab, index) => tab.setAttribute("aria-selected", String(index === state.tab)));
  ui.overview.hidden = state.tab !== 0;
  ui.settings.hidden = state.tab !== 1;
}

function renderBusy() {
  document.body.classList.toggle("running", state.busy);
  ui.sync.textContent = state.busy ? "Abbrechen" : "Jetzt synchronisieren";
  const gameActive = state.game !== "idle";
  ui.sync.disabled = (state.busy && !state.cancellable) || (!state.busy && gameActive);
  ui.refresh.disabled = state.busy || gameActive;
  ui.join.disabled = state.busy || gameActive;
  ui.join.textContent = { idle: "Direkt beitreten", starting: "Spiel startet …", running: "Spiel läuft" }[state.game];
  ui.save.disabled = state.busy;
  $<HTMLButtonElement>("#import-server").disabled = state.busy;
  $<HTMLButtonElement>("#armm-disconnect").disabled = state.busy;
}

function renderOverview() {
  const profile = selected();
  if (!profile) {
    return;
  }
  ui.title.textContent = displayName(profile);
  const address = combinedAddress(profile.draft) ?? "";
  const dir = profile.draft.downloadDir.trim() || "Standardordner des Spiels";
  // Ohne geholte Liste gibt es keinen Stand, dann auch keinen Trenner am Ende.
  ui.hint.textContent = [`Adresse: ${address}`, `Zielordner: ${dir}`, profile.stand].filter((part) => part.trim()).join("  ·  ");
  ui.summary.textContent = profile.summary;
  ui.sync.classList.toggle("attention", (profile.overview?.open ?? 0) > 0);
  ui.progress.style.width = `${Math.min(100, (profile.progressDone / profile.progressTotal) * 100)}%`;
  ui.status.textContent = profile.status;
  ui.status.classList.toggle("error", profile.statusIsError);

  const rows = profile.overview?.rows ?? [];
  ui.rows.replaceChildren(
    ...rows.map((row) => {
      const tr = document.createElement("tr");
      for (const text of [row.name, row.required, row.local]) {
        tr.append(Object.assign(document.createElement("td"), { textContent: text, title: text }));
      }
      const state = Object.assign(document.createElement("td"), { textContent: row.state });
      state.classList.toggle("open", row.isOpen && !row.isDamaged);
      state.classList.toggle("damaged", row.isDamaged);
      tr.append(state);
      return tr;
    }),
  );
  renderLog();
}

function renderLog() {
  const profile = selected();
  const atEnd = ui.log.scrollTop + ui.log.clientHeight >= ui.log.scrollHeight - 4;
  ui.log.textContent = profile?.log ?? "";
  if (atEnd) {
    ui.log.scrollTop = ui.log.scrollHeight;
  }
}

/** Füllt das Formular. Nur beim Wechsel des Servers, sonst ginge die Eingabe des Nutzers verloren. */
function fillForm() {
  const profile = selected();
  if (!profile) {
    return;
  }
  ui.name.value = profile.draft.name;
  ui.ip.value = profile.draft.ip;
  ui.port.value = profile.draft.port;
  ui.dir.value = profile.draft.downloadDir;
  // Ein getipptes Passwort darf nie beim nächsten Server landen.
  ui.password.value = "";
  renderSettingsState();
}

function renderSettingsState() {
  const profile = selected();
  if (!profile) {
    return;
  }
  const showSaved = profile.hasPassword && !profile.editingPassword;
  ui.pwSaved.hidden = !showSaved;
  ui.pwEdit.hidden = showSaved;
  ui.pwCancel.hidden = !(profile.hasPassword && profile.editingPassword);
  ui.pwHint.textContent = showSaved
    ? "Sicher im Windows-Anmeldeinformationsspeicher abgelegt."
    : profile.hasPassword
      ? "Neues Passwort eingeben und unten speichern. Es ersetzt das bisherige."
      : "Leer lassen, wenn der Server kein Passwort hat.";
  ui.dirExtra.hidden = !profile.draft.downloadDir.trim();
  const armm = profile.saved?.armm ?? null;
  ui.armmSection.hidden = !armm;
  ui.addressHint.textContent = armm ? ARMM_ADDRESS_HINT : SERVER_ADDRESS_HINT;
  if (armm) {
    ui.armmTarget.textContent = `Modset ${armm.modsetId} auf ${armm.host}`;
    ui.armmKey.textContent = armm.hasAccessKey ? "✓ Schlüssel gespeichert" : "Schlüssel fehlt, bitte neu importieren";
    ui.armmKey.classList.toggle("error", !armm.hasAccessKey);
  }
  ui.settingsMessage.textContent = profile.settingsMessage;
  ui.settingsMessage.classList.toggle("error", profile.settingsIsError);
}

function renderJoinNotice() {
  ui.joinNotice.hidden = state.game === "idle";
  ui.joinNotice.classList.toggle("running", state.game === "running");
  const server = state.joinName ? `„${state.joinName}“` : "dem Server";
  if (state.game === "starting") {
    ui.joinNoticeTitle.textContent = "Arma Reforger wird gestartet …";
    ui.joinNoticeText.textContent = `Das Spiel tritt danach ${server} bei. Bis das Fenster erscheint, kann es bis zu einer Minute dauern. Bitte nicht erneut klicken.`;
  } else if (state.game === "running") {
    ui.joinNoticeTitle.textContent = "Arma Reforger läuft";
    ui.joinNoticeText.textContent = state.joinName
      ? `Das Spiel tritt ${server} bei. Klappt das nicht, bleibt es im Hauptmenü, und du kannst selbst beitreten.`
      : "Beitreten und Synchronisieren gehen erst wieder, wenn das Spiel geschlossen ist.";
  }
}

function render() {
  renderList();
  renderTabs();
  renderBusy();
  renderOverview();
  renderJoinNotice();
  renderSettingsState();
}

// --- Aktionen ---

async function select(id: string) {
  if (state.busy || id === state.selectedId) {
    return;
  }
  const leaving = selected();
  if (leaving) {
    leaving.editingPassword = false;
  }
  state.selectedId = id;
  fillForm();
  render();
  void refreshSteamOptions();
  if (selected()?.saved) {
    await api.selectProfile(id).catch(() => undefined);
  }
}

function addProfile() {
  const profile = newProfile(null);
  setSettingsMessage(profile, "Bitte Adresse und Passwort des Spielservers eintragen.", false);
  state.profiles.push(profile);
  state.selectedId = profile.id;
  state.tab = 1;
  fillForm();
  render();
  void refreshSteamOptions();
  ui.name.focus();
}

/** Legt einen Server aus einer armm-sync.json an. Den Zugangsschlüssel liest nur das Backend. */
async function importServer() {
  if (state.busy) {
    return;
  }
  let preview;
  try {
    preview = await api.pickArmmImport();
  } catch (problem) {
    await informDialog("Importdatei nicht verwendbar", String(problem));
    return;
  }
  if (!preview) {
    return;
  }

  const lines = [
    `Name: ${preview.name}`,
    `ARMM: ${preview.armmHost}, Modset ${preview.modsetId}`,
    `Spielserver: ${preview.gameServer ?? "nicht angegeben, bitte danach in den Einstellungen eintragen"}`,
  ];
  if (preview.passwordRequired) {
    lines.push("", "Der Spielserver hat ein Passwort. Du gibst es danach in den Einstellungen ein, es steht nie in der Datei.");
  }
  if (preview.existing) {
    lines.push("", `„${preview.existing}“ ist schon mit diesem Modset verbunden und bekommt nur den neuen Zugangsschlüssel. Name, Adresse, Passwort und Zielordner bleiben.`);
  }
  const confirmed = await confirmDialog(
    preview.existing ? "ARMM-Verbindung aktualisieren?" : "Server aus ARMM importieren?",
    lines.join("\n"),
    preview.existing ? "Aktualisieren" : "Importieren",
  );
  if (!confirmed) {
    await api.cancelArmmImport();
    return;
  }

  let result;
  try {
    result = await api.applyArmmImport();
  } catch (problem) {
    await informDialog("Import fehlgeschlagen", String(problem));
    return;
  }

  let profile = state.profiles.find((candidate) => candidate.id === result.profile.id);
  if (profile) {
    profile.saved = result.profile;
  } else {
    profile = newProfile(result.profile);
    // Ein leerer, nie gespeicherter Platzhalter wird durch den importierten Server ersetzt.
    const placeholder = state.profiles.length === 1 && !state.profiles[0].saved && !hasContent(state.profiles[0]);
    if (placeholder) {
      state.profiles = [profile];
    } else {
      state.profiles.push(profile);
    }
  }
  state.selectedId = profile.id;
  state.tab = 1;
  profile.editingPassword = result.needsPassword;
  const extra = result.needsAddress ? " Die Importdatei nennt keinen Spielserver: Bitte Adresse und Port eintragen." : "";
  setSettingsMessage(profile, result.message + extra, false);
  fillForm();
  render();
  void refreshSteamOptions();
  if (result.needsAddress) {
    ui.ip.focus();
  } else if (result.needsPassword) {
    ui.password.focus();
  }
  await api.selectProfile(profile.id).catch(() => undefined);
}

function hasContent(profile: Profile): boolean {
  const draft = profile.draft;
  return Boolean(draft.name.trim() || draft.ip.trim() || draft.port.trim() || draft.downloadDir.trim());
}

async function disconnectArmm() {
  const profile = selected();
  if (!profile?.saved?.armm || state.busy) {
    return;
  }
  const confirmed = await confirmDialog(
    "Verbindung zu ARMM trennen?",
    "Der Server bleibt erhalten und holt seine Mod-Liste danach wieder vom laufenden Spielserver. Der gespeicherte Zugangsschlüssel wird gelöscht.",
    "Trennen",
    { danger: true },
  );
  if (!confirmed) {
    return;
  }
  try {
    profile.saved = await api.disconnectArmm(profile.id);
    resetOverview(profile);
    setSettingsMessage(profile, "Verbindung zu ARMM getrennt. Die Mod-Liste kommt jetzt vom Spielserver.", false);
  } catch (problem) {
    setSettingsMessage(profile, String(problem), true);
  }
  render();
}

async function removeProfile() {
  const profile = selected();
  if (!profile || state.busy) {
    return;
  }
  const confirmed = await confirmDialog(
    `Server „${displayName(profile)}“ entfernen?`,
    profile.saved?.armm
      ? "Der Eintrag, sein gespeichertes Passwort und der ARMM-Zugangsschlüssel werden gelöscht. Die geladenen Mods bleiben auf der Festplatte."
      : "Der Eintrag und sein gespeichertes Passwort werden gelöscht. Die geladenen Mods bleiben auf der Festplatte.",
    "Entfernen",
    { danger: true },
  );
  if (!confirmed) {
    return;
  }
  if (profile.saved) {
    try {
      await api.removeProfile(profile.id);
    } catch (problem) {
      await informDialog("Server konnte nicht entfernt werden", String(problem));
      return;
    }
  }
  const index = state.profiles.indexOf(profile);
  state.profiles.splice(index, 1);
  if (state.profiles.length === 0) {
    state.profiles.push(newProfile(null));
  }
  state.selectedId = state.profiles[Math.min(index, state.profiles.length - 1)].id;
  fillForm();
  render();
}

async function saveProfile(event: SubmitEvent) {
  event.preventDefault();
  const profile = selected();
  if (!profile || state.busy) {
    return;
  }

  const password = ui.password.value;
  let keepPassword = false;
  if (await api.passwordTransferNeeded(toInput(profile), password.length > 0)) {
    keepPassword = await confirmDialog(
      "Server-Adresse geändert",
      "Das gespeicherte Passwort gehört zur bisherigen Adresse. Soll es auch für die neue Adresse gelten? Es wird dann für diesen Server verwendet, sobald du ihn prüfst, synchronisierst oder ihm beitrittst. Ohne Übernahme wird es gelöscht.",
      "Übernehmen",
      { cancelText: "Nicht übernehmen" },
    );
  }

  const addressChanged = profile.saved !== null && profile.saved.serverAddress !== combinedAddress(profile.draft);
  ui.save.disabled = true;
  try {
    const result = await api.saveProfile(toInput(profile), password, keepPassword);
    if (result.profile) {
      profile.saved = result.profile;
      profile.hasPassword = result.profile.hasPassword;
      if (addressChanged) {
        resetOverview(profile);
      } else if (result.profile.overview) {
        showOverview(profile, result.profile.overview);
      }
    }
    // Nach einem Fehlschlag bleibt die Eingabe stehen, damit sie nicht neu getippt werden muss.
    if (result.stored) {
      ui.password.value = "";
      profile.editingPassword = false;
    }
    setSettingsMessage(profile, result.message, result.isError);
    render();
  } finally {
    ui.save.disabled = state.busy;
  }
}

/** Prüft, ob gestartet werden darf, und meldet sonst warum. */
function readyToRun(profile: Profile): boolean {
  if (!profile.saved || hasUnsavedChanges(profile)) {
    setStatus(profile, "Dieser Server hat ungespeicherte Änderungen. Bitte zuerst in den Einstellungen speichern.", true);
    renderOverview();
    return false;
  }
  return true;
}

function beginRun(cancellable: boolean) {
  state.busy = true;
  state.cancellable = cancellable;
  render();
}

function endRun() {
  state.busy = false;
  state.cancellable = false;
  render();
}

function applyRunResult(profile: Profile, result: RunResult) {
  if (result.overview) {
    showOverview(profile, result.overview);
  } else {
    profile.listStatus = result.listStatus;
  }
  setStatus(profile, result.message, result.isError);
}

async function refresh() {
  const profile = selected();
  if (!profile || state.busy) {
    return;
  }
  if (!profile.saved) {
    setStatus(profile, "Bitte den Server zuerst in den Einstellungen speichern.", true);
    renderOverview();
    return;
  }
  profile.listStatus = "wird geprüft …";
  setStatus(profile, profile.saved.armm ? "Mod-Liste wird aus ARMM geholt …" : "Mod-Liste wird vom Server geholt …", false);
  beginRun(false);
  try {
    applyRunResult(profile, await api.checkProfile(profile.id));
  } catch (problem) {
    setStatus(profile, String(problem), true);
  } finally {
    endRun();
  }
}

async function syncOrCancel() {
  if (state.busy) {
    if (!state.cancellable) {
      return;
    }
    const running = selected();
    if (running) {
      setStatus(running, "Wird abgebrochen, der Spielclient wird beendet …", false);
      renderOverview();
    }
    await api.cancelRun();
    return;
  }

  const profile = selected();
  if (!profile || !readyToRun(profile)) {
    return;
  }

  profile.log = "";
  profile.listStatus = "synchronisiert …";
  setStatus(profile, "Wird gestartet …", false);
  beginRun(true);
  try {
    const result = await api.syncProfile(profile.id);
    appendLog(profile, result.message);
    applyRunResult(profile, result);
    if (result.cleanupQuestion) {
      state.cancellable = false;
      render();
      const approved = await confirmDialog(result.cleanupQuestion.title, result.cleanupQuestion.text, "Löschen", {
        danger: true,
        cancelText: "Behalten",
      });
      const answer = await api.answerCleanup(profile.id, approved);
      appendLog(profile, answer.message);
      setStatus(profile, answer.message, answer.isError);
    }
  } catch (problem) {
    setStatus(profile, String(problem), true);
  } finally {
    endRun();
  }
}

async function join() {
  const profile = selected();
  if (!profile || state.busy || state.game !== "idle" || !readyToRun(profile)) {
    return;
  }

  // Erst frisch abgleichen: Seit dem letzten Prüfen kann sich das Modset oder der Zielordner geändert haben.
  setStatus(profile, profile.saved?.armm ? "Mod-Liste wird vor dem Beitritt aus ARMM geholt …" : "Mods werden vor dem Beitritt geprüft …", false);
  beginRun(false);
  let checked: RunResult | null = null;
  try {
    checked = await api.prepareJoin(profile.id);
    applyRunResult(profile, checked);
  } catch (problem) {
    setStatus(profile, String(problem), true);
  } finally {
    endRun();
  }
  if (!checked || checked.cancelled) {
    return;
  }

  const open = profile.overview?.open ?? 0;
  if (open > 0) {
    const title = open === 1 ? "1 Mod passt nicht zum Server" : `${open} Mods passen nicht zum Server`;
    if (!(await confirmDialog(title, "Der Beitritt kann daran scheitern. Am besten zuerst synchronisieren.", "Trotzdem beitreten"))) {
      return;
    }
  }
  // Sofort sperren: Bis das Backend antwortet, darf kein zweiter Klick durchkommen.
  state.game = "starting";
  state.joinName = displayName(profile);
  render();
  try {
    const result = await api.joinServer(profile.id);
    setStatus(profile, result.message, result.isError);
    if (result.isError) {
      state.game = "idle";
      state.joinName = "";
    } else {
      watchGame();
    }
  } catch (problem) {
    setStatus(profile, String(problem), true);
    state.game = "idle";
    state.joinName = "";
  }
  render();
}

const GAME_START_LIMIT_MS = 90_000;
let gameWatch: number | undefined;

/** Verfolgt nach dem Beitritt, ob das Spiel startet und wann es wieder geschlossen ist. */
function watchGame() {
  window.clearTimeout(gameWatch);
  const startedAt = Date.now();
  const tick = async () => {
    const running = await api.gameRunning().catch(() => false);
    if (running) {
      state.game = "running";
    } else if (state.game === "running" || Date.now() - startedAt > GAME_START_LIMIT_MS) {
      const timedOut = state.game === "starting";
      state.game = "idle";
      state.joinName = "";
      const profile = selected();
      if (timedOut && profile) {
        setStatus(profile, "Das Spiel ist nach 90 Sekunden noch nicht gestartet. Bitte Steam prüfen und erneut beitreten.", true);
      }
    }
    render();
    if (state.game !== "idle") {
      gameWatch = window.setTimeout(() => void tick(), state.game === "running" ? 5000 : 2000);
    }
  };
  gameWatch = window.setTimeout(() => void tick(), 2000);
}

function onRunStatus(status: RunStatus) {
  const profile = state.profiles.find((candidate) => candidate.id === status.profileId);
  if (!profile) {
    return;
  }
  appendLog(profile, status.message);
  if (!status.isClientLine) {
    profile.status = status.message;
    profile.statusIsError = false;
  }
  if (status.overview) {
    showOverview(profile, status.overview);
    profile.listStatus = "synchronisiert …";
  }
  if (profile.id === state.selectedId) {
    renderOverview();
  }
  renderList();
}

// --- Eingaben ---

function bindInputs() {
  const update = (change: (draft: Draft) => void) => {
    const profile = selected();
    if (!profile) {
      return;
    }
    change(profile.draft);
    renderList();
    renderOverview();
    renderSettingsState();
  };

  ui.name.addEventListener("input", () => update((draft) => (draft.name = ui.name.value)));

  // Wer aus Gewohnheit Adresse:Port tippt, landet mit dem Doppelpunkt im Portfeld. Eingefügtes Adresse:Port
  // oder eine ganze URL wird verteilt. Getippt werden dürfen nur Zeichen, die in Namen und IP-Adressen vorkommen.
  ui.ip.addEventListener("beforeinput", (event) => {
    if (event.data === ":") {
      event.preventDefault();
      ui.port.focus();
      ui.port.select();
    } else if (event.inputType === "insertText" && event.data && !/^[A-Za-z\d.-]+$/.test(event.data)) {
      event.preventDefault();
    }
  });
  ui.ip.addEventListener("input", () => {
    const raw = ui.ip.value.replace(/\s+/g, "");
    const clean = /[:/?#]/.test(raw) ? normalizeAddress(raw) : raw.toLowerCase();
    if (clean.includes(":")) {
      const [ip, port] = splitAddress(clean);
      ui.ip.value = ip;
      if (port) {
        ui.port.value = port.replace(/\D/g, "").slice(0, 5);
      }
    } else if (clean !== ui.ip.value) {
      ui.ip.value = clean;
    }
    update((draft) => {
      draft.ip = ui.ip.value;
      draft.port = ui.port.value;
    });
  });
  ui.port.addEventListener("input", () => {
    const clean = ui.port.value.replace(/\D/g, "").slice(0, 5);
    if (clean !== ui.port.value) {
      ui.port.value = clean;
    }
    update((draft) => (draft.port = clean));
  });

  ui.dir.addEventListener("input", () => {
    update((draft) => (draft.downloadDir = ui.dir.value));
    void refreshSteamOptions();
  });

  $("#browse-dir").addEventListener("click", async () => {
    const chosen = await api.browseFolder(ui.dir.value);
    if (chosen) {
      ui.dir.value = chosen;
      ui.dir.dispatchEvent(new Event("input"));
    }
  });

  $("#copy-options").addEventListener("click", () => navigator.clipboard.writeText(ui.steamOptions.value));

  $("#pw-change").addEventListener("click", () => {
    const profile = selected();
    if (profile) {
      profile.editingPassword = true;
      renderSettingsState();
      ui.password.focus();
    }
  });
  ui.pwCancel.addEventListener("click", () => {
    const profile = selected();
    ui.password.value = "";
    if (profile) {
      profile.editingPassword = false;
      renderSettingsState();
    }
  });
  $("#pw-delete").addEventListener("click", async () => {
    const profile = selected();
    if (!profile) {
      return;
    }
    const confirmed = await confirmDialog(
      "Gespeichertes Passwort löschen?",
      "Für diesen Server musst du das Passwort danach neu eingeben.",
      "Löschen",
      { danger: true },
    );
    if (!confirmed) {
      return;
    }
    try {
      const result = await api.deletePassword(profile.id);
      profile.hasPassword = false;
      ui.password.value = "";
      setSettingsMessage(profile, result.message, result.isError);
    } catch (problem) {
      setSettingsMessage(profile, String(problem), true);
    }
    renderSettingsState();
  });

  ui.form.addEventListener("submit", (event) => void saveProfile(event));
}

async function refreshSteamOptions() {
  const dir = ui.dir.value;
  ui.steamOptions.value = dir.trim() ? await api.steamOptions(dir) : "";
}

function bindSidebar() {
  ui.serverList.addEventListener("click", (event) => {
    const item = (event.target as HTMLElement).closest("li");
    if (item?.dataset.id) {
      void select(item.dataset.id);
    }
  });
  ui.serverList.addEventListener("keydown", (event) => {
    const index = state.profiles.findIndex((profile) => profile.id === state.selectedId);
    const next = event.key === "ArrowDown" ? index + 1 : event.key === "ArrowUp" ? index - 1 : -2;
    if (next >= 0 && next < state.profiles.length) {
      event.preventDefault();
      void select(state.profiles[next].id).then(() => (ui.serverList.children[next] as HTMLElement | undefined)?.focus());
    }
  });
  $("#add-server").addEventListener("click", addProfile);
  $("#import-server").addEventListener("click", () => void importServer());
  $("#armm-disconnect").addEventListener("click", () => void disconnectArmm());
  $("#remove-server").addEventListener("click", () => void removeProfile());
  $("#basic-settings").addEventListener("click", () => void basicSettingsDialog());
  document.querySelectorAll<HTMLAnchorElement>("[data-link]").forEach((link) =>
    link.addEventListener("click", (event) => {
      event.preventDefault();
      api.openLink(link.dataset.link as "repository" | "releases").catch((problem) =>
        informDialog("Browser konnte nicht geöffnet werden", String(problem)),
      );
    }),
  );
  ui.tabs.forEach((tab, index) =>
    tab.addEventListener("click", () => {
      state.tab = index;
      renderTabs();
    }),
  );
  ui.sync.addEventListener("click", () => void syncOrCancel());
  ui.refresh.addEventListener("click", () => void refresh());
  ui.join.addEventListener("click", () => void join());
}

// --- Updates ---

let foundUpdate: UpdateView | null = null;

function showUpdate(update: UpdateView) {
  foundUpdate = update;
  $("#update").hidden = false;
  $("#update-text").textContent = update.canInstall
    ? `Version ${update.version} ist verfügbar.`
    : `Version ${update.version} ist verfügbar. Mit dem Installer von der Download-Seite aktualisiert sich die App künftig selbst.`;
  const button = $<HTMLButtonElement>("#update-button");
  button.textContent = update.canInstall ? "Jetzt aktualisieren" : "Zur Download-Seite";
  button.disabled = false;
}

/** Beim Start still, nur ein gefundenes Update zeigt sich. Von Hand mit Antwort in jedem Fall. */
async function checkForUpdate(manual: boolean) {
  try {
    const update = await api.checkUpdate();
    if (update) {
      showUpdate(update);
    } else if (manual) {
      await informDialog("Kein Update", "Die App ist auf dem neuesten Stand.");
    }
  } catch (problem) {
    if (manual) {
      await informDialog("Suche nach Updates fehlgeschlagen", String(problem));
    }
  }
}

async function installUpdate() {
  const update = foundUpdate;
  if (!update) {
    return;
  }
  if (!update.canInstall) {
    await api.openLink("releases").catch((problem) => informDialog("Browser konnte nicht geöffnet werden", String(problem)));
    return;
  }
  const confirmed = await confirmDialog(
    `Auf Version ${update.version} aktualisieren?`,
    "Die App lädt das Update, beendet sich und startet danach neu. Einstellungen, Passwörter und Mods bleiben erhalten.",
    "Aktualisieren",
  );
  if (!confirmed) {
    return;
  }
  const button = $<HTMLButtonElement>("#update-button");
  button.disabled = true;
  button.textContent = "Wird geladen …";
  try {
    // Kehrt nur bei einem Fehler zurück, sonst übernimmt der Installer.
    await api.installUpdate();
  } catch (problem) {
    await informDialog("Update fehlgeschlagen", String(problem));
    showUpdate(update);
  }
}

function onUpdateProgress(progress: UpdateProgress) {
  const percent = progress.total ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : null;
  $("#update-button").textContent = percent === null ? "Wird geladen …" : `Wird geladen … ${percent} %`;
}

function bindUpdate() {
  $("#update-button").addEventListener("click", () => void installUpdate());
  $("#update-check").addEventListener("click", (event) => {
    event.preventDefault();
    void checkForUpdate(true);
  });
}

/** Kein Kontextmenü des Browsers und kein Neuladen der Seite, das wirkt in einer App fremd. */
function disableBrowserBehaviour() {
  document.addEventListener("contextmenu", (event) => {
    if (!(event.target instanceof HTMLInputElement)) {
      event.preventDefault();
    }
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "F5" || (event.ctrlKey && event.key.toLowerCase() === "r")) {
      event.preventDefault();
    }
  });
}

async function start() {
  disableBrowserBehaviour();
  bindSidebar();
  bindInputs();
  bindUpdate();

  const app = await api.loadApp();
  ui.version.textContent = `Version ${app.version}`;
  document.title = `Reforger Sync & Launch ${app.version}`;
  state.profiles = app.profiles.map(newProfile);
  if (state.profiles.length === 0) {
    state.profiles.push(newProfile(null));
  }
  const first = state.profiles.find((profile) => profile.id === app.selectedProfileId) ?? state.profiles[0];
  state.selectedId = first.id;

  const configured = first.saved !== null && combinedAddress(first.draft) !== null;
  if (app.loadProblem) {
    setSettingsMessage(first, app.loadProblem, true);
  } else if (!configured) {
    setSettingsMessage(first, "Bitte Adresse und Passwort des Spielservers eintragen.", false);
  }
  state.tab = configured ? 0 : 1;

  await listen<RunStatus>("run-status", (event) => onRunStatus(event.payload));
  await listen<UpdateProgress>("update-progress", (event) => onUpdateProgress(event.payload));
  await listen("close-requested", async () => {
    const quit = await confirmDialog(
      "Vorgang läuft noch",
      "Beim Beenden wird der laufende Vorgang abgebrochen. Bereits geladene Mods bleiben erhalten.",
      "Beenden",
    );
    if (quit) {
      await api.quitApp();
    }
  });

  fillForm();
  await refreshSteamOptions();
  render();
  void checkForUpdate(false);
  // Läuft das Spiel schon beim Start der App, sind Beitritt und Sync gleich gesperrt.
  if (await api.gameRunning().catch(() => false)) {
    state.game = "running";
    render();
    watchGame();
  }
}

start().catch((problem) => informDialog("Unerwarteter Fehler", String(problem)));
