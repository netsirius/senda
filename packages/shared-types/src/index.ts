// Phase 0/1: hand-written IPC types. Phase 2 swaps these for tauri-specta
// auto-generated bindings (output: ./bindings.ts). The Rust types in
// `crates/core` are the single source of truth either way.

export type AgentCli = "copilot" | "claude-code" | "gemini";

export interface Greeting {
  agentName: string;
  agentVersion: string;
}

export type AgentSource =
  | { kind: "personal" }
  | { kind: "external"; originalCli: AgentCli }
  | { kind: "repo"; repoId: number; path: string };

export interface McpServerSpec {
  type: string;
  command?: string;
  args?: string[];
  url?: string;
  env?: Record<string, string>;
}

export interface CopilotSpecific {
  target?: string;
}

export interface ClaudeCodeSpecific {
  permissionMode?: string;
  hooks?: Record<string, string>;
}

export interface GeminiSpecific {
  model?: string;
}

export interface CanonicalAgent {
  name: string;
  description: string;
  targets: AgentCli[];
  tools: string[];
  "mcp-servers": Record<string, McpServerSpec>;
  copilot?: CopilotSpecific;
  "claude-code"?: ClaudeCodeSpecific;
  gemini?: GeminiSpecific;
  body: string;
}

export interface Agent {
  id: string;
  agent: CanonicalAgent;
  source: AgentSource;
  canonicalPath: string | null;
  warningsCount: number;
}

export type CatalogEntry =
  | ({ kind: "agent" } & Agent)
  | {
      kind: "error";
      id: string;
      path: string;
      source: AgentSource;
      message: string;
    };
