import { createMemo, For, Show, type Component } from "solid-js";
import { mcps, refetchMcps } from "../stores/discovery";
import type { AgentCli } from "senda-shared-types";

const CLI_LABEL: Record<AgentCli, string> = {
  copilot: "Copilot",
  "claude-code": "Claude Code",
  gemini: "Gemini",
};

const Mcps: Component = () => {
  const grouped = createMemo(() => {
    const by: Record<AgentCli, typeof mcps extends () => infer T ? T : never> = {
      copilot: [],
      "claude-code": [],
      gemini: [],
    } as never;
    for (const m of mcps() ?? []) {
      (by[m.cli] as any).push(m);
    }
    return by;
  });

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>MCP servers</h1>
            <p class="page-subtitle">
              Read straight from each CLI's config:
              <code>~/.copilot/mcp-servers.json</code>,{" "}
              <code>~/.claude.json</code> / <code>~/.claude/settings.json</code>,{" "}
              <code>~/.gemini/settings.json</code>. Senda doesn't install or remove MCPs — that's
              your CLI's job.
            </p>
          </div>
          <button class="btn-secondary" onClick={() => refetchMcps()}>
            Refresh
          </button>
        </div>
      </header>

      <Show
        when={(mcps() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <h2>No MCPs detected</h2>
            <p class="muted">
              Configure MCPs through your CLI of choice and they'll appear here. Examples:{" "}
              <code>copilot mcp add</code>, <code>claude mcp add</code>.
            </p>
          </div>
        }
      >
        <For each={Object.entries(grouped()) as [AgentCli, typeof mcps extends () => infer T ? T : never][]}>
          {([cli, items]) => (
            <Show when={(items as any[]).length > 0}>
              <section class="detail-block">
                <h2>{CLI_LABEL[cli]}</h2>
                <table class="mcp-table">
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th>Type</th>
                      <th>Command / URL</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={items as any[]}>
                      {(m) => (
                        <tr>
                          <td>
                            <code>{m.name}</code>
                          </td>
                          <td>{m.type}</td>
                          <td>
                            <code>{m.command ?? m.url ?? "—"}</code>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </section>
            </Show>
          )}
        </For>
      </Show>
    </section>
  );
};

export default Mcps;
