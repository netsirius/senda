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
              Cosas que un automation iba a hacer y están esperando tu OK.
            </p>
          </div>
          <button class="btn-secondary" onClick={() => refetchPending()}>
            Refresh
          </button>
        </div>
      </header>

      <Show
        when={(pending() ?? []).length > 0}
        fallback={<EmptyExplainer />}
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
                  Agent: <code>{run.agentId}</code> · queued{" "}
                  {new Date(run.queuedAt * 1000).toLocaleString()}
                </p>
                <p class="muted small">Prompt que se mandaría al agente:</p>
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

const EmptyExplainer: Component = () => (
  <section class="detail-block">
    <h2>¿Qué es esta pantalla?</h2>
    <p class="muted small">
      Cuando creas una automation con <strong>Approval gate</strong> activado, en lugar de
      ejecutar el agente al disparar el trigger, Senda <strong>pausa</strong> y lo deja aquí.
      Tú lees el prompt exacto que se enviaría al agente y decides si lo apruebas.
    </p>

    <h3 style="margin-top:16px">Para qué sirve</h3>
    <ul class="muted small" style="margin:0; padding-left:18px">
      <li>
        Tareas que escriben en sistemas externos (cerrar tickets, enviar emails, mergear PRs)
        que prefieres revisar antes.
      </li>
      <li>
        Probar un agente nuevo en producción sin riesgo: ves qué <em>habría</em> hecho cada vez.
      </li>
      <li>
        Auditoría: cada aprobación queda registrada en <code>automation_runs</code>.
      </li>
    </ul>

    <h3 style="margin-top:16px">Cómo crear una automation con approval</h3>
    <ol class="muted small" style="margin:0; padding-left:18px">
      <li>Ve a <A href="/automations/new">/automations/new</A></li>
      <li>Step 4 (Guards) → marca <strong>Approval gate</strong></li>
      <li>Cuando el cron / webhook / event dispare, aparecerá aquí</li>
      <li>Click <strong>Approve & run</strong> y el agente ejecuta de verdad</li>
    </ol>

    <p class="muted small" style="margin-top:16px">
      <strong>Hoy no hay nada pendiente.</strong> O no tienes ninguna automation con
      approval gate, o ninguna ha disparado todavía.
    </p>
  </section>
);

export default Approvals;
