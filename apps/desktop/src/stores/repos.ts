import { createResource, createRoot } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface RepoRow {
  id: number;
  provider: string;
  org: string;
  project: string | null;
  repo: string;
  url: string;
  localPath: string;
  branch: string;
  authKind: string;
  authKeyringId: string | null;
  autoSync: boolean;
  syncIntervalSeconds: number;
  lastSyncedAt: number | null;
  lastSyncError: string | null;
}

async function fetchRepos(): Promise<RepoRow[]> {
  return await invoke<RepoRow[]>("list_repos");
}

function createReposStore() {
  const [repos, { refetch, mutate }] = createResource(fetchRepos);

  // Auto-refresh whenever the backend signals a change.
  void listen("repos:changed", () => refetch());
  void listen("repos:synced", () => refetch());

  return { repos, refetch, mutate };
}

export const { repos, refetch: refetchRepos, mutate: mutateRepos } = createRoot(createReposStore);
