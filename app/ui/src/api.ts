// Aufrufe ins Rust-Backend. Passwörter gehen nur bei saveProfile hinein und kommen nie zurück.
// ARMM-Zugangsschlüssel sieht die Oberfläche nie, das Backend liest sie selbst aus der Importdatei.
import { invoke } from "@tauri-apps/api/core";

export interface ModRow {
  name: string;
  required: string;
  local: string;
  state: string;
  isOpen: boolean;
  isDamaged: boolean;
}

export interface ModSummary {
  total: number;
  current: number;
  listStatus: string;
  text: string;
}

export interface Overview {
  rows: ModRow[];
  summary: ModSummary;
  open: number;
  stand: string;
}

export interface ProfileView {
  id: string;
  name: string;
  serverAddress: string | null;
  downloadDir: string | null;
  hasPassword: boolean;
  overview: Overview | null;
  armm: ArmmView | null;
}

export interface ArmmView {
  host: string;
  modsetId: number;
  hasAccessKey: boolean;
}

export interface ImportPreview {
  name: string;
  armmHost: string;
  modsetId: number;
  gameServer: string | null;
  passwordRequired: boolean;
  existing: string | null;
}

export interface ImportResult {
  profile: ProfileView;
  created: boolean;
  needsPassword: boolean;
  needsAddress: boolean;
  message: string;
}

export interface AppView {
  version: string;
  profiles: ProfileView[];
  selectedProfileId: string | null;
  loadProblem: string | null;
}

export interface ProfileInput {
  id: string;
  name: string;
  ip: string;
  port: string;
  downloadDir: string;
}

export interface Message {
  message: string;
  isError: boolean;
}

export interface SaveProfileResult extends Message {
  stored: boolean;
  profile: ProfileView | null;
}

export interface RunStatus {
  profileId: string;
  message: string;
  isClientLine: boolean;
  done: number;
  total: number;
  overview: Overview | null;
}

export interface RunResult extends Message {
  ok: boolean;
  cancelled: boolean;
  listStatus: string;
  overview: Overview | null;
  cleanupQuestion: { title: string; text: string } | null;
}

export interface BasicSettingsInput {
  reforgerExe: string;
  totalMinutes: string;
  idleMinutes: string;
}

export interface BasicSettingsView {
  input: BasicSettingsInput;
  defaults: BasicSettingsInput;
  detectedExeHint: string;
}

export type BasicSettingsField = keyof BasicSettingsInput;

export interface BasicSettingsSaveResult {
  ok: boolean;
  errors: Partial<Record<BasicSettingsField, string>>;
  message: string;
}

export interface UpdateView {
  version: string;
  canInstall: boolean;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export const api = {
  loadApp: () => invoke<AppView>("load_app"),
  selectProfile: (id: string | null) => invoke<void>("select_profile", { id }),
  passwordTransferNeeded: (input: ProfileInput, passwordEntered: boolean) =>
    invoke<boolean>("password_transfer_needed", { input, passwordEntered }),
  saveProfile: (input: ProfileInput, password: string, keepPassword: boolean) =>
    invoke<SaveProfileResult>("save_profile", { input, password, keepPassword }),
  removeProfile: (id: string) => invoke<void>("remove_profile", { id }),
  deletePassword: (id: string) => invoke<Message>("delete_password", { id }),
  steamOptions: (downloadDir: string) => invoke<string>("steam_options", { downloadDir }),
  checkProfile: (id: string) => invoke<RunResult>("check_profile", { id }),
  syncProfile: (id: string) => invoke<RunResult>("sync_profile", { id }),
  answerCleanup: (id: string, approved: boolean) => invoke<Message>("answer_cleanup", { id, approved }),
  cancelRun: () => invoke<void>("cancel_run"),
  joinServer: (id: string) => invoke<Message>("join_server", { id }),
  gameRunning: () => invoke<boolean>("game_running"),
  basicSettings: () => invoke<BasicSettingsView>("basic_settings"),
  saveBasicSettings: (input: BasicSettingsInput) => invoke<BasicSettingsSaveResult>("save_basic_settings", { input }),
  browseFolder: (current: string) => invoke<string | null>("browse_folder", { current }),
  browseExe: (current: string) => invoke<string | null>("browse_exe", { current }),
  openLink: (target: "repository" | "releases") => invoke<void>("open_link", { target }),
  pickArmmImport: () => invoke<ImportPreview | null>("pick_armm_import"),
  cancelArmmImport: () => invoke<void>("cancel_armm_import"),
  applyArmmImport: () => invoke<ImportResult>("apply_armm_import"),
  disconnectArmm: (id: string) => invoke<ProfileView>("disconnect_armm", { id }),
  quitApp: () => invoke<void>("quit_app"),
  checkUpdate: () => invoke<UpdateView | null>("check_update"),
  installUpdate: () => invoke<void>("install_update"),
};
