import { For, Show, type Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { A } from "@solidjs/router";

import { pending, refetchPending, refetchCount, type PendingRun } from "../stores/approvals";

const Approvals: Component = () => {
  const approve = async (run: PendingRun) => {
    if (!confirm(`Approve and run automation "${run.automationName}"?`)) return;
    try {
      await invoke("approve_pending_run", { runId: run.id });
      await refetchPending();
      await refetchCount();
    } catch (e) {
      alert(`Approve failed: ${e}`);
    }
  };

  const reject = async (run: PendingRun) => {
    if (!confirm(`Reject this pending run? It won't fire.`)) return;
    try {
      await invoke("reject_pending_run", { runId: run.id });
      await refetchPending();
      await refetchCount();
    } catch (e) {
      alert(`Reject failed: ${e}`);
    }
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>Pending approvals</h1>
            <p class="page-subtitle">
              Automations with the <code>approval_gate</code> guard pause here instead of running.
              Approve to dispatch the agent with the queued prompt; reject to discard.
            </p>
          </div>
          <button class="btn-secondary" onClick={() => refetchPending()}>
            Refresh
          </button>
        </div>
      </header>

      <Show
        when={(pending() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <p class="muted">
              Nothing pending. Create an automation with{" "}
              <strong>Approval gate</strong> on (Step 4 of <A href="/automations/new">New automation</A>) to queue
              runs here.
            </p>
          </div>
        }
      >
        <div class="agent-grid">
          <For each={pending()}>
            {(run) => (
              <article class="agent-card">
                <header class="agent-card-header">
                  <h3>{run.automationName}</h3>
                  <span class="status status--running">awaiting</span>
                </header>
                <p class="agent-card-desc">
                  Agent: <code>{run.agentId}</code> · queued {new Date(run.queuedAt * 1000).toLocaleString()}
                </p>
                <pre class="run-output">{run.prompt || "(empty prompt)"}</pre>
                <footer class="agent-card-footer agent-card-actions">
                  <button class="btn-primary" onClick={() => approve(run)}>
                    Approve & run
                  </button>
                  <button class="btn-danger" onClick={() => reject(run)}>
                    Reject
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

export default Approvals;
