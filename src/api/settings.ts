import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "@/types/models";

export function getAppSettings() {
  return invoke<AppSettings>("get_app_settings");
}

export function saveAppSettings(settings: AppSettings) {
  return invoke<AppSettings>("save_app_settings", { settings });
}
