import { createResource, createRoot } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
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

  return { mcps, builtinTools, skills, refetchMcps, refetchSkills };
}

export const { mcps, builtinTools, skills, refetchMcps, refetchSkills } =
  createRoot(createDiscoveryStore);
