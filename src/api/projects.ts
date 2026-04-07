import { invoke } from "@tauri-apps/api/core";
import type {
  NormalizedDocument,
  ProjectDetail,
  ProjectSummary,
  SourceType,
} from "@/types/models";

export function listProjects() {
  return invoke<ProjectSummary[]>("list_projects");
}

export function importDocument(filePath: string) {
  return invoke<ProjectSummary>("import_document", { filePath });
}

export function getProjectDetail(projectId: string) {
  return invoke<ProjectDetail | null>("get_project_detail", { projectId });
}

export function importNormalizedDocument(
  filePath: string,
  sourceType: SourceType,
  normalizedDocument: NormalizedDocument,
) {
  return invoke<ProjectSummary>("import_normalized_document", {
    filePath,
    sourceType,
    normalizedDocument,
  });
}
