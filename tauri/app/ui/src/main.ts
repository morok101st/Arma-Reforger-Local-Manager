import { invoke } from "@tauri-apps/api/core";

const version = document.querySelector<HTMLSpanElement>("#version");
if (version) {
  version.textContent = await invoke<string>("app_version");
}
