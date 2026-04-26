import { createResource, createRoot } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentCli } from "senda-shared-types";

export interface InstalledMcp {
  cli: AgentCli;
  name: string;
  type: string;
  command: string | null;
  url: string | null;
}

export interface CliTools {
  cli: AgentCli;
  tools: string[];
}

export interface SkillEntry {
  cli: AgentCli;
  name: string;
  path: string;
  description: string | null;
}

function createDiscoveryStore() {
  const [mcps, { refetch: refetchMcps }] = createResource(() =>
    invoke<InstalledMcp[]>("list_installed_mcps"),
  );
  const [builtinTools] = createResource(() => invoke<CliTools[]>("list_builtin_tools"));
  const [skills, { refetch: refetchSkills }] = createResource(() =>
    invoke<SkillEntry[]>("list_skills"),
  );

  // Auto-refresh after the user mutates something through Senda. External
  // mutations still require a manual Refresh click — fs-watching the CLI
  // configs is a future enhancement.
  void listen("mcps:changed", () => refetchMcps());
  void listen("skills:changed", () => refetchSkills());

  return { mcps, builtinTools, skills, refetchMcps, refetchSkills };
}

export const { mcps, builtinTools, skills, refetchMcps, refetchSkills } =
  createRoot(createDiscoveryStore);
