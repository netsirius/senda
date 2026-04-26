import { createMemo, createResource, For, Show, type Component } from "solid-js";
import { A, useNavigate, useParams } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import type { AgentCli, McpServerSpec } from "senda-shared-types";

import { catalog as entries, refetchCatalog } from "../stores/catalog";
import { renderMarkdown } from "../lib/markdown";

interface ExecutionRow {
  id: string;
  agentId: string;
  cli: string;
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  promptHash: string;
  dryRun: boolean;
}

const CLI_LABEL: Record<AgentCli, string> = {
  copilot: "Copilot",
  "claude-code": "Claude Code",
  gemini: "Gemini",
};

const CLI_CHIP: Record<AgentCli, string> = {
  copilot: "chip chip-copilot",
  "claude-code": "chip chip-claude",
  gemini: "chip chip-gemini",
};

/** Mask anything that looks like a secret reference or env-var template. */
function maskValue(value: string): string {
  if (value.includes("${secret:") || value.includes("${env:") || /token|secret|key|password/i.test(value)) {
    return "•••";
  }
  return value;
}

const AgentDetail: Component = () => {
  const params = useParams();
  const navigate = useNavigate();

  const targetEntry = createMemo(() => {
    const id = decodeURIComponent(params.id ?? "");
    const list = entries() ?? [];
    return list.find((e) => e.kind === "agent" && e.id === id);
  });

  const agent = createMemo(() => {
    const e = targetEntry();
    return e && e.kind === "agent" ? e.agent : null;
  });

  const sourceLabel = createMemo(() => {
    const e = targetEntry();
    if (!e || e.kind !== "agent") return "—";
    if (e.source.kind === "personal") return "Personal";
    if (e.source.kind === "external") return `External · ${CLI_LABEL[e.source.originalCli]}`;
    return `Repo · ${e.source.repoId}`;
  });

  const isExternal = createMemo(
    () => targetEntry()?.kind === "agent" && targetEntry()!.source.kind === "external",
  );

  const renderedBody = createMemo(() => {
    const a = agent();
    return a ? renderMarkdown(a.body || "_No prompt body._") : "";
  });

  const [recentRuns] = createResource(
    () => targetEntry()?.kind === "agent" ? targetEntry()!.id : null,
    (id) => (id ? invoke<ExecutionRow[]>("list_executions_for_agent", { agentId: id, limit: 10 }) : Promise.resolve([])),
  );

  const reveal = (path: string) => invoke("reveal_in_finder", { path });

  const mcpServers = createMemo(() => {
    const a = agent();
    if (!a) return [] as Array<[string, McpServerSpec]>;
    return Object.entries(a["mcp-servers"]) as Array<[string, McpServerSpec]>;
  });

  return (
    <Show
      when={targetEntry()}
      fallback={
        <section class="catalog">
          <header class="page-header">
            <h1>Agent not found</h1>
            <p class="page-subtitle">
              <A href="/">← Back to catalog</A>
            </p>
          </header>
        </section>
      }
    >
      {(_) => (
        <section class="agent-detail">
          <nav class="breadcrumbs">
            <A href="/">Catalog</A>
            <span class="dot">›</span>
            <span>{agent()!.name}</span>
          </nav>

          <header class="page-header agent-detail-header">
            <div>
              <h1>{agent()!.name}</h1>
              <p class="page-subtitle">{agent()!.description || "—"}</p>
              <p class="muted small">{sourceLabel()}</p>
            </div>
            <div class="agent-detail-actions">
              <button
                class="btn-primary"
                onClick={() => navigate(`/agent/run/${encodeURIComponent(targetEntry()!.id)}`)}
                disabled={agent()!.targets.length === 0}
                title={
                  agent()!.targets.length === 0
                    ? "This agent has no `targets:` declared. Edit it and add at least one CLI."
                    : ""
                }
              >
                Run agent
              </button>
              <button
                class="btn-secondary"
                disabled={isExternal()}
                title={
                  isExternal()
                    ? "External agents are read-only. Use 'Import to Senda' from the catalog to copy it into ~/.senda/agents/ first."
                    : ""
                }
                onClick={() => {
                  const entry = targetEntry();
                  const path =
                    entry && entry.kind === "agent" ? entry.canonicalPath : null;
                  navigate(path ? `/agent/edit?path=${encodeURIComponent(path)}` : "/agent/edit");
                }}
              >
                Edit
              </button>
              <button
                class="btn-danger"
                onClick={async () => {
                  const a = agent();
                  if (!a) {
                    alert("No agent loaded — try Refresh from the catalog.");
                    return;
                  }
                  const e = targetEntry();
                  const external =
                    e && e.kind === "agent" && e.source.kind === "external";
                  const message = external
                    ? `"${a.name}" lives outside Senda (in your CLI's native folder). Delete the native file?`
                    : `Delete "${a.name}"? This removes the canonical document and any generated per-CLI artefacts.`;
                  if (!confirm(message)) {
                    return;
                  }
                  try {
                    await invoke("delete_agent", { name: a.name, draft: false });
                    await refetchCatalog();
                    navigate("/");
                  } catch (err) {
                    alert(`Delete failed: ${err}`);
                  }
                }}
              >
                Delete
              </button>
            </div>
          </header>

          <section class="detail-block">
            <h2>Targets</h2>
            <ul class="chip-row">
              <For each={agent()!.targets}>
                {(target) => <li class={CLI_CHIP[target]}>{CLI_LABEL[target]}</li>}
              </For>
            </ul>
          </section>

          <section class="detail-block">
            <h2>Tools</h2>
            <Show when={agent()!.tools.length > 0} fallback={<p class="muted">None declared.</p>}>
              <ul class="chip-row">
                <For each={agent()!.tools}>{(t) => <li class="chip chip-tool">{t}</li>}</For>
              </ul>
            </Show>
          </section>

          <section class="detail-block">
            <h2>MCP servers</h2>
            <Show
              when={mcpServers().length > 0}
              fallback={<p class="muted">No MCP servers declared.</p>}
            >
              <table class="mcp-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Type</th>
                    <th>Command / URL</th>
                    <th>Env</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={mcpServers()}>
                    {([name, spec]) => (
                      <tr>
                        <td>
                          <code>{name}</code>
                        </td>
                        <td>{spec.type}</td>
                        <td>
                          <code>{spec.command ?? spec.url ?? "—"}</code>
                        </td>
                        <td>
                          <Show
                            when={spec.env && Object.keys(spec.env).length > 0}
                            fallback={<span class="muted">—</span>}
                          >
                            <ul class="env-list">
                              <For each={Object.entries(spec.env ?? {})}>
                                {([k, v]) => (
                                  <li>
                                    <code>{k}</code>={" "}
                                    <code class="masked">{maskValue(v)}</code>
                                  </li>
                                )}
                              </For>
                            </ul>
                          </Show>
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </Show>
          </section>

          <section class="detail-block">
            <h2>Per-CLI configuration</h2>
            <CliConfigPanel
              copilot={agent()!.copilot}
              claudeCode={agent()!["claude-code"]}
              gemini={agent()!.gemini}
            />
          </section>

          <section class="detail-block">
            <h2>Prompt</h2>
            <article class="markdown-body" innerHTML={renderedBody()} />
          </section>

          <section class="detail-block">
            <h2>Recent runs</h2>
            <Show
              when={(recentRuns() ?? []).length > 0}
              fallback={<p class="muted">No runs yet — kick one off with the Run button above.</p>}
            >
              <table class="mcp-table">
                <thead>
                  <tr>
                    <th>Started</th>
                    <th>CLI</th>
                    <th>Duration</th>
                    <th>Exit</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={recentRuns()}>
                    {(r) => (
                      <tr>
                        <td>{new Date(r.startedAt * 1000).toLocaleString()}</td>
                        <td><code>{r.cli}</code></td>
                        <td>
                          {r.endedAt
                            ? `${r.endedAt - r.startedAt}s`
                            : <span class="muted">running…</span>}
                        </td>
                        <td>
                          <Show when={r.exitCode != null} fallback={<span class="muted">—</span>}>
                            <span class={r.exitCode === 0 ? "status status--succeeded" : "status status--failed"}>
                              {r.exitCode}
                            </span>
                          </Show>
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </Show>
          </section>

          <Show
            when={(() => {
              const e = targetEntry();
              return e && e.kind === "agent" ? e.canonicalPath : null;
            })()}
          >
            {(canonicalPath) => (
              <section class="detail-block">
                <h2>Files on disk</h2>
                <p class="muted small">Click to reveal in your file manager.</p>
                <ul class="env-list">
                  <li>
                    <button
                      class="btn-secondary small"
                      onClick={() => reveal(canonicalPath())}
                    >
                      📂 {canonicalPath()}
                    </button>
                  </li>
                </ul>
              </section>
            )}
          </Show>
        </section>
      )}
    </Show>
  );
};

interface CliConfigProps {
  copilot?: { target?: string };
  claudeCode?: { permissionMode?: string; hooks?: Record<string, string> };
  gemini?: { model?: string };
}

const CliConfigPanel: Component<CliConfigProps> = (props) => {
  const empty =
    !props.copilot && !props.claudeCode && !props.gemini;

  if (empty) {
    return <p class="muted">No CLI-specific configuration.</p>;
  }

  return (
    <div class="cli-config-grid">
      <Show when={props.copilot}>
        <div class="cli-config-card">
          <h3>Copilot</h3>
          <dl>
            <Show when={props.copilot!.target}>
              <dt>target</dt>
              <dd>
                <code>{props.copilot!.target}</code>
              </dd>
            </Show>
          </dl>
        </div>
      </Show>
      <Show when={props.claudeCode}>
        <div class="cli-config-card">
          <h3>Claude Code</h3>
          <dl>
            <Show when={props.claudeCode!.permissionMode}>
              <dt>permissionMode</dt>
              <dd>
                <code>{props.claudeCode!.permissionMode}</code>
              </dd>
            </Show>
            <Show when={props.claudeCode!.hooks && Object.keys(props.claudeCode!.hooks).length > 0}>
              <dt>hooks</dt>
              <dd>
                <ul class="env-list">
                  <For each={Object.entries(props.claudeCode!.hooks ?? {})}>
                    {([k, v]) => (
                      <li>
                        <code>{k}</code>: <code>{v}</code>
                      </li>
                    )}
                  </For>
                </ul>
              </dd>
            </Show>
          </dl>
        </div>
      </Show>
      <Show when={props.gemini}>
        <div class="cli-config-card">
          <h3>Gemini</h3>
          <dl>
            <Show when={props.gemini!.model}>
              <dt>model</dt>
              <dd>
                <code>{props.gemini!.model}</code>
              </dd>
            </Show>
          </dl>
        </div>
      </Show>
    </div>
  );
};

export default AgentDetail;
