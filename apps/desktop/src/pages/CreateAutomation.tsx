import { createMemo, createSignal, For, Show, type Component } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import { catalog as entries } from "../stores/catalog";

type TriggerKind = "schedule" | "event" | "webhook" | "manual";

const CreateAutomation: Component = () => {
  const navigate = useNavigate();
  const [step, setStep] = createSignal<"agent" | "trigger" | "guards" | "review">("agent");
  const [name, setName] = createSignal("my-automation");
  const [agentId, setAgentId] = createSignal("");
  const [triggerKind, setTriggerKind] = createSignal<TriggerKind>("manual");
  const [cronExpr, setCronExpr] = createSignal("0 9 * * 1");
  const [timezone, setTimezone] = createSignal("UTC");
  const [webhookPath, setWebhookPath] = createSignal("hello");
  const [webhookSecret, setWebhookSecret] = createSignal("");
  const [idempotency, setIdempotency] = createSignal(true);
  const [rateLimit, setRateLimit] = createSignal(100);
  const [approvalGate, setApprovalGate] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const availableAgents = createMemo(() =>
    (entries() ?? []).filter((e) => e.kind === "agent"),
  );

  const buildTrigger = () => {
    switch (triggerKind()) {
      case "schedule":
        return { kind: "schedule", cron: cronExpr(), timezone: timezone() };
      case "webhook":
        return {
          kind: "webhook",
          path: webhookPath(),
          secret: webhookSecret() ? webhookSecret() : null,
        };
      case "event":
        return {
          kind: "event",
          mcp: "stub",
          eventFilter: {},
          pollIntervalSeconds: 60,
        };
      case "manual":
      default:
        return { kind: "manual" };
    }
  };

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("create_automation", {
        args: {
          name: name(),
          agentId: agentId(),
          trigger: buildTrigger(),
          guards: {
            idempotency: idempotency(),
            rateLimitPerHour: rateLimit(),
            approvalGate: approvalGate(),
            backpressure: "skip",
          },
          enabled: true,
        },
      });
      navigate("/automations");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <h1>New automation</h1>
        <p class="page-subtitle">Pick an agent, a trigger, the guards, then review.</p>
      </header>

      <ol class="stepper">
        <li classList={{ active: step() === "agent", done: step() !== "agent" }}>1. Agent</li>
        <li
          classList={{
            active: step() === "trigger",
            done: step() === "guards" || step() === "review",
          }}
        >
          2. Trigger
        </li>
        <li classList={{ active: step() === "guards", done: step() === "review" }}>3. Guards</li>
        <li classList={{ active: step() === "review" }}>4. Review</li>
      </ol>

      <Show when={step() === "agent"}>
        <section class="detail-block">
          <div class="settings-row">
            <label>Automation name</label>
            <input value={name()} onInput={(e) => setName(e.currentTarget.value)} />
          </div>
          <div class="settings-row">
            <label>Agent</label>
            <select value={agentId()} onChange={(e) => setAgentId(e.currentTarget.value)}>
              <option value="" disabled>
                Pick an agent…
              </option>
              <For each={availableAgents()}>
                {(e) =>
                  e.kind === "agent" ? (
                    <option value={e.id}>
                      {e.agent.name} ({e.source.kind})
                    </option>
                  ) : null
                }
              </For>
            </select>
          </div>
          <div class="step-actions">
            <button class="btn-primary" disabled={!agentId()} onClick={() => setStep("trigger")}>
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "trigger"}>
        <section class="detail-block">
          <div class="provider-grid">
            <For each={["schedule", "webhook", "manual"] as TriggerKind[]}>
              {(t) => (
                <button
                  class="provider-card"
                  classList={{ active: triggerKind() === t }}
                  onClick={() => setTriggerKind(t)}
                >
                  <h3>{t}</h3>
                  <p>
                    {t === "schedule"
                      ? "Cron expression with timezone."
                      : t === "webhook"
                        ? "POST hits to localhost:9876/hook/<path>."
                        : "Run manually from the UI."}
                  </p>
                </button>
              )}
            </For>
          </div>

          <Show when={triggerKind() === "schedule"}>
            <div class="settings-row">
              <label>Cron</label>
              <input value={cronExpr()} onInput={(e) => setCronExpr(e.currentTarget.value)} />
              <p class="muted small">Example: <code>0 9 * * 1</code> = every Monday at 09:00.</p>
            </div>
            <div class="settings-row">
              <label>Timezone</label>
              <input value={timezone()} onInput={(e) => setTimezone(e.currentTarget.value)} />
            </div>
          </Show>

          <Show when={triggerKind() === "webhook"}>
            <div class="settings-row">
              <label>Path segment</label>
              <input value={webhookPath()} onInput={(e) => setWebhookPath(e.currentTarget.value)} />
              <p class="muted small">
                Webhook URL: <code>http://localhost:9876/hook/{webhookPath()}</code>
              </p>
            </div>
            <div class="settings-row">
              <label>HMAC secret (optional)</label>
              <input
                type="password"
                value={webhookSecret()}
                onInput={(e) => setWebhookSecret(e.currentTarget.value)}
              />
              <p class="muted small">
                Sign requests with <code>x-senda-signature: sha256=&lt;hex&gt;</code>.
              </p>
            </div>
          </Show>

          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("agent")}>
              Back
            </button>
            <button class="btn-primary" onClick={() => setStep("guards")}>
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "guards"}>
        <section class="detail-block">
          <div class="settings-row">
            <label>
              <input
                type="checkbox"
                checked={idempotency()}
                onChange={(e) => setIdempotency(e.currentTarget.checked)}
              />{" "}
              Idempotency
            </label>
            <p class="muted small">Skip events that have already been processed.</p>
          </div>
          <div class="settings-row">
            <label>Rate limit (runs per hour)</label>
            <input
              type="number"
              value={rateLimit()}
              onInput={(e) => setRateLimit(Number(e.currentTarget.value))}
            />
          </div>
          <div class="settings-row">
            <label>
              <input
                type="checkbox"
                checked={approvalGate()}
                onChange={(e) => setApprovalGate(e.currentTarget.checked)}
              />{" "}
              Approval gate
            </label>
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("trigger")}>
              Back
            </button>
            <button class="btn-primary" onClick={() => setStep("review")}>
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "review"}>
        <section class="detail-block">
          <h2>Review</h2>
          <pre class="markdown-body">
            {JSON.stringify(
              {
                name: name(),
                agentId: agentId(),
                trigger: buildTrigger(),
                guards: {
                  idempotency: idempotency(),
                  rateLimitPerHour: rateLimit(),
                  approvalGate: approvalGate(),
                  backpressure: "skip",
                },
              },
              null,
              2,
            )}
          </pre>
          <Show when={error()}>
            <p class="error-banner">{error()}</p>
          </Show>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("guards")} disabled={busy()}>
              Back
            </button>
            <button class="btn-primary" onClick={submit} disabled={busy()}>
              {busy() ? "Creating…" : "Activate"}
            </button>
          </div>
        </section>
      </Show>
    </section>
  );
};

export default CreateAutomation;
