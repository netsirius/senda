import { createMemo, createResource, createSignal, For, Show, type Component } from "solid-js";
import { A } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

interface AutomationRunRow {
  id: number;
  automationId: number | null;
  startedAt: number;
  endedAt: number | null;
  status: string;
  outputText: string | null;
  errorText: string | null;
  dryRun: boolean;
}

const formatDate = (ts: number) => new Date(ts * 1000).toLocaleString();
const formatDuration = (start: number, end: number | null) => {
  if (end == null) return "running…";
  const seconds = end - start;
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
};

type Tab = "manual" | "automation";

const History: Component = () => {
  const [tab, setTab] = createSignal<Tab>("automation");
  const [manual, manualMeta] = createResource(() =>
    invoke<ExecutionRow[]>("list_executions", { limit: 100 }),
  );
  const [autos, autoMeta] = createResource(() =>
    invoke<AutomationRunRow[]>("list_recent_automation_runs", { limit: 100 }),
  );
  const [showOutput, setShowOutput] = createSignal<number | null>(null);

  // Auto-refresh on relevant Tauri events.
  void listen("automation:fired", () => autoMeta.refetch());

  const refresh = () => {
    void manualMeta.refetch();
    void autoMeta.refetch();
  };

  const counts = createMemo(() => ({
    manual: manual()?.length ?? 0,
    automation: autos()?.length ?? 0,
  }));

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>History</h1>
            <p class="page-subtitle">
              All agent runs. Manual = you clicked Run; Automation = a trigger fired.
            </p>
          </div>
          <button class="btn-secondary" onClick={refresh}>
            Refresh
          </button>
        </div>
        <nav class="catalog-tabs">
          <button
            class="catalog-tab"
            classList={{ active: tab() === "automation" }}
            onClick={() => setTab("automation")}
          >
            Automation <span class="catalog-tab-count">{counts().automation}</span>
          </button>
          <button
            class="catalog-tab"
            classList={{ active: tab() === "manual" }}
            onClick={() => setTab("manual")}
          >
            Manual <span class="catalog-tab-count">{counts().manual}</span>
          </button>
        </nav>
      </header>

      <Show when={tab() === "automation"}>
        <Show
          when={(autos() ?? []).length > 0}
          fallback={<EmptyState what="automation runs" />}
        >
          <table class="mcp-table">
            <thead>
              <tr>
                <th>Started</th>
                <th>Automation</th>
                <th>Status</th>
                <th>Duration</th>
                <th>Mode</th>
                <th>Output</th>
              </tr>
            </thead>
            <tbody>
              <For each={autos()}>
                {(row) => (
                  <>
                    <tr>
                      <td>{formatDate(row.startedAt)}</td>
                      <td>#{row.automationId ?? "?"}</td>
                      <td>
                        <span class={`status status--${statusClass(row.status)}`}>
                          {row.status}
                        </span>
                      </td>
                      <td>{formatDuration(row.startedAt, row.endedAt)}</td>
                      <td>{row.dryRun ? "dry-run" : "live"}</td>
                      <td>
                        <button
                          class="btn-secondary small"
                          onClick={() =>
                            setShowOutput(showOutput() === row.id ? null : row.id)
                          }
                        >
                          {showOutput() === row.id ? "Hide" : "Show"}
                        </button>
                      </td>
                    </tr>
                    <Show when={showOutput() === row.id}>
                      <tr>
                        <td colspan="6">
                          <Show
                            when={row.outputText || row.errorText}
                            fallback={<span class="muted">(no output)</span>}
                          >
                            <pre class="run-output">
                              {row.errorText ? `ERROR:\n${row.errorText}\n\n` : ""}
                              {row.outputText ?? ""}
                            </pre>
                          </Show>
                        </td>
                      </tr>
                    </Show>
                  </>
                )}
              </For>
            </tbody>
          </table>
        </Show>
      </Show>

      <Show when={tab() === "manual"}>
        <Show
          when={(manual() ?? []).length > 0}
          fallback={<EmptyState what="manual runs" />}
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
              </tr>
            </thead>
            <tbody>
              <For each={manual()}>
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
                      <Show
                        when={row.exitCode != null}
                        fallback={<span class="muted">—</span>}
                      >
                        <span
                          class={
                            row.exitCode === 0
                              ? "status status--succeeded"
                              : "status status--failed"
                          }
                        >
                          {row.exitCode}
                        </span>
                      </Show>
                    </td>
                    <td>{row.dryRun ? "dry-run" : "live"}</td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </Show>
      </Show>
    </section>
  );
};

const EmptyState: Component<{ what: string }> = (props) => (
  <div class="empty-state">
    <p class="muted">No {props.what} yet.</p>
  </div>
);

function statusClass(status: string): string {
  switch (status) {
    case "success":
      return "succeeded";
    case "failed":
      return "failed";
    case "running":
      return "running";
    default:
      return "cancelled";
  }
}

export default History;
