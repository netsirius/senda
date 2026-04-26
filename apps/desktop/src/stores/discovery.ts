import { createResource, createRoot, createSignal } from "solid-js";
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

interface McpToolList {
  mcp: string;
  tools: string[];
}

function createDiscoveryStore() {
  const [mcps, { refetch: refetchMcps }] = createResource(() =>
    invoke<InstalledMcp[]>("list_installed_mcps"),
  );
  const [builtinTools] = createResource(() => invoke<CliTools[]>("list_builtin_tools"));
  const [skills, { refetch: refetchSkills }] = createResource(() =>
    invoke<SkillEntry[]>("list_skills"),
  );

  // Cache of mcp-name → tools[]. Populated on demand so we don't spawn every
  // declared MCP at startup. Cleared when the MCP set changes.
  const [mcpTools, setMcpTools] = createSignal<Record<string, string[]>>({});

  void listen("mcps:changed", () => {
    void refetchMcps();
    setMcpTools({});
  });
  void listen("skills:changed", () => refetchSkills());

  /**
   * Returns the tools exposed by an MCP, spawning it once and caching the
   * result. Falls back to an empty list on failure — the caller can then
   * default to a `<mcp>/` prefix in autocomplete.
   */
  async function fetchMcpTools(name: string): Promise<string[]> {
    const cached = mcpTools()[name];
    if (cached) return cached;
    try {
      const result = await invoke<McpToolList>("introspect_mcp_tools", { name });
      setMcpTools((prev) => ({ ...prev, [name]: result.tools }));
      return result.tools;
    } catch {
      // Swallow — autocomplete still has the prefix to offer.
      return [];
    }
  }

  return {
    mcps,
    builtinTools,
    skills,
    mcpTools,
    refetchMcps,
    refetchSkills,
    fetchMcpTools,
  };
}

export const {
  mcps,
  builtinTools,
  skills,
  mcpTools,
  refetchMcps,
  refetchSkills,
  fetchMcpTools,
} = createRoot(createDiscoveryStore);
