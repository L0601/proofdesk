import { invoke } from "@tauri-apps/api/core";
import type { ProjectSummary } from "@/types/models";

export function listProjects() {
  return invoke<ProjectSummary[]>("list_projects");
}

export function importDocument(filePath: string) {
  return invoke<ProjectSummary>("import_document", { filePath });
}
