import { invoke } from "@tauri-apps/api/core";
import type {
  ProofreadOptions,
  ProofreadingCall,
  ProofreadingIssue,
  ProofreadingJob,
} from "@/types/models";

export function startProofreading(
  projectId: string,
  options: ProofreadOptions,
) {
  return invoke<ProofreadingJob>("start_proofreading", {
    projectId,
    options,
  });
}

export function getLatestProofreadingJob(projectId: string) {
  return invoke<ProofreadingJob | null>("get_latest_proofreading_job", {
    projectId,
  });
}

export function pauseProofreading(projectId: string) {
  return invoke<ProofreadingJob | null>("pause_proofreading", {
    projectId,
  });
}

export function listProofreadingIssues(projectId: string) {
  return invoke<ProofreadingIssue[]>("list_proofreading_issues", {
    projectId,
  });
}

export function listProofreadingCalls(projectId: string) {
  return invoke<ProofreadingCall[]>("list_proofreading_calls", {
    projectId,
  });
}
