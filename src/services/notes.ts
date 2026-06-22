import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types/note";

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function updateSettings(settings: Settings): Promise<void> {
  return invoke("update_settings", { newSettings: settings });
}
