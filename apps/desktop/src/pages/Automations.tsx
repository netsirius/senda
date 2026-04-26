import { createMemo, createResource, For, Show, type Component } from "solid-js";
import { A } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface AutomationRow {
  id: number;
  name: string;
  agentId: string;
  triggerKind: string;
  triggerConfig: string;
  guards: string;
  promptTemplate: string | null;
  enabled: boolean;
  createdAt: number;
  lastRunAt: number | null;
  lastRunStatus: string | null;
}

async function fetchAutomations(): Promise<AutomationRow[]> {
  return await invoke<AutomationRow[]>("list_automations");
}

const Automations: Component = () => {
  const [rows, { refetch }] = createResource(fetchAutomations);

  void listen("automations:changed", () => refetch());
  void listen("automation:fired", () => refetch());

  const counts = createMemo(() => {
    const list = rows() ?? [];
    return {
      all: list.length,
      active: list.filter((a) => a.enabled).length,
      paused: list.filter((a) => !a.enabled).length,
    };
  });

  const toggle = async (row: AutomationRow) => {
    await invoke("set_automation_enabled", { id: row.id, enabled: !row.enabled });
    await refetch();
  };

  const remove = async (row: AutomationRow) => {
    if (!confirm(`Delete automation "${row.name}"?`)) return;
    await invoke("delete_automation", { id: row.id });
    await refetch();
  };

  const runNow = async (row: AutomationRow) => {
    await invoke("run_automation_now", { id: row.id, dryRun: false });
  };

  const testWebhook = async (row: AutomationRow) => {
    try {
      const config = JSON.parse(row.triggerConfig);
      const path = config.path;
      if (!path) return;
      const status = await invoke<number>("webhook_self_test", {
        path,
        body: prompt("Body to POST (JSON):", '{"source":"senda-self-test"}') ?? null,
      });
      alert(`POST returned ${status}.`);
      await refetch();
    } catch (e) {
      alert(`Test failed: ${e}`);
    }
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>Automations</h1>
            <p class="page-subtitle">
              Cron, webhook, and manual triggers. Scheduler runs while Senda is open; webhook
              endpoint is{" "}
              <code>http://localhost:9876/hook/&lt;path&gt;</code>.
            </p>
          </div>
          <div class="catalog-header-actions">
            <A class="btn-primary" href="/automations/new">
              New automation
            </A>
            <A class="btn-secondary" href="/automations/new">
              📚 Browse templates
            </A>
          </div>
        </div>

        <div class="catalog-tabs">
          <span class="catalog-tab">All <span class="catalog-tab-count">{counts().all}</span></span>
          <span class="catalog-tab">Active <span class="catalog-tab-count">{counts().active}</span></span>
          <span class="catalog-tab">Paused <span class="catalog-tab-count">{counts().paused}</span></span>
        </div>
      </header>

      <Show
        when={(rows() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <h2>No automations yet</h2>
            <p class="muted">
              Create one to run agents on a cron, react to webhooks, or kick off manual jobs.
            </p>
          </div>
        }
      >
        <div class="agent-grid">
          <For each={rows()}>
            {(row) => (
              <article class="agent-card">
                <header class="agent-card-header">
                  <h3>{row.name}</h3>
                  <span
                    class={`status status--${row.enabled ? "succeeded" : "cancelled"}`}
                    title={row.enabled ? "Active" : "Paused"}
                  >
                    {row.enabled ? "active" : "paused"}
                  </span>
                </header>
                <p class="agent-card-desc">
                  Agent: <code>{row.agentId}</code>
                </p>
                <ul class="chip-row">
                  <li class="chip chip-tool">{row.triggerKind}</li>
                  <Show when={row.lastRunStatus}>
                    <li class="chip chip-tool">last: {row.lastRunStatus}</li>
                  </Show>
                </ul>
                <footer class="agent-card-footer agent-card-actions">
                  <button class="btn-secondary" onClick={() => runNow(row)}>
                    Run now
                  </button>
                  <Show when={row.triggerKind === "webhook"}>
                    <button class="btn-secondary" onClick={() => testWebhook(row)}>
                      Test webhook
                    </button>
                  </Show>
                  <button class="btn-secondary" onClick={() => toggle(row)}>
                    {row.enabled ? "Pause" : "Resume"}
                  </button>
                  <button class="btn-danger" onClick={() => remove(row)}>
                    Delete
                  </button>
                </footer>
              </article>
            )}
          </For>
        </div>
      </Show>
    </section>
  );
};

export default Automations;
