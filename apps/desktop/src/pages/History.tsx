import { createResource, For, Show, type Component } from "solid-js";
import { A } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";

interface ExecutionRow {
  id: string;
  agentId: string;
  agentSource: string;
  cli: string;
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  promptHash: string;
  cwd: string | null;
  dryRun: boolean;
}

async function fetchExecutions(): Promise<ExecutionRow[]> {
  return await invoke<ExecutionRow[]>("list_executions", { limit: 100 });
}

const formatDate = (ts: number) => new Date(ts * 1000).toLocaleString();
const formatDuration = (start: number, end: number | null) => {
  if (end == null) return "running…";
  const seconds = end - start;
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
};

const History: Component = () => {
  const [rows, { refetch }] = createResource(fetchExecutions);

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>History</h1>
            <p class="page-subtitle">
              Recent agent executions. Prompts are hashed (SHA-256) and never stored in plain text.
            </p>
          </div>
          <button class="btn-secondary" onClick={() => refetch()}>
            Refresh
          </button>
        </div>
      </header>

      <Show
        when={!rows.loading && (rows() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <p class="muted">{rows.loading ? "Loading…" : "No executions yet."}</p>
          </div>
        }
      >
        <table class="mcp-table">
          <thead>
            <tr>
              <th>Started</th>
              <th>Agent</th>
              <th>CLI</th>
              <th>Duration</th>
              <th>Exit</th>
              <th>Mode</th>
              <th>Prompt hash</th>
            </tr>
          </thead>
          <tbody>
            <For each={rows()}>
              {(row) => (
                <tr>
                  <td>{formatDate(row.startedAt)}</td>
                  <td>
                    <A href={`/agent/${encodeURIComponent(row.agentId)}`}>{row.agentId}</A>
                  </td>
                  <td>
                    <code>{row.cli}</code>
                  </td>
                  <td>{formatDuration(row.startedAt, row.endedAt)}</td>
                  <td>
                    <Show when={row.exitCode != null} fallback={<span class="muted">—</span>}>
                      <span class={row.exitCode === 0 ? "status status--succeeded" : "status status--failed"}>
                        {row.exitCode}
                      </span>
                    </Show>
                  </td>
                  <td>{row.dryRun ? "dry-run" : "live"}</td>
                  <td>
                    <code class="agent-card-path">{row.promptHash.slice(0, 12)}…</code>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </Show>
    </section>
  );
};

export default History;
