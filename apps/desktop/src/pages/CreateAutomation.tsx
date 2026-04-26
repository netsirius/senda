import { createMemo, createSignal, For, Show, type Component } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import { catalog as entries } from "../stores/catalog";
import { TEMPLATES, type AutomationTemplate } from "../data/automation-templates";

type TriggerKind = "schedule" | "event" | "webhook" | "manual";

const CreateAutomation: Component = () => {
  const navigate = useNavigate();
  const [step, setStep] = createSignal<"agent" | "trigger" | "prompt" | "guards" | "review">(
    "agent",
  );
  const [name, setName] = createSignal("my-automation");
  const [agentId, setAgentId] = createSignal("");
  const [triggerKind, setTriggerKind] = createSignal<TriggerKind>("manual");
  const [cronExpr, setCronExpr] = createSignal("0 9 * * 1");
  const [timezone, setTimezone] = createSignal("UTC");
  const [webhookPath, setWebhookPath] = createSignal("hello");
  const [webhookSecret, setWebhookSecret] = createSignal("");
  const [mcpName, setMcpName] = createSignal("");
  const [pollInterval, setPollInterval] = createSignal(60);
  const [promptTemplate, setPromptTemplate] = createSignal("");
  const [idempotency, setIdempotency] = createSignal(true);
  const [rateLimit, setRateLimit] = createSignal(100);
  const [approvalGate, setApprovalGate] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const availableAgents = createMemo(() =>
    (entries() ?? []).filter((e) => e.kind === "agent"),
  );

  const applyTemplate = (t: AutomationTemplate) => {
    setName(t.id);
    setTriggerKind(t.trigger.kind);
    if (t.trigger.cron) setCronExpr(t.trigger.cron);
    if (t.trigger.timezone) setTimezone(t.trigger.timezone);
    if (t.trigger.path) setWebhookPath(t.trigger.path);
    if (t.trigger.secret !== undefined) setWebhookSecret(t.trigger.secret);
    if (t.trigger.mcp !== undefined) setMcpName(t.trigger.mcp);
    if (t.trigger.pollIntervalSeconds !== undefined)
      setPollInterval(t.trigger.pollIntervalSeconds);
    setPromptTemplate(t.promptTemplate);
    setIdempotency(t.guards.idempotency);
    setRateLimit(t.guards.rateLimitPerHour);
    setApprovalGate(t.guards.approvalGate);
    if (t.starterAgent) {
      // Pre-select the matching agent if it already exists in the catalog,
      // otherwise navigate to the editor pre-filled so the user can save it
      // and come back. We don't auto-create agents — they need explicit
      // intent.
      const existing = availableAgents().find(
        (e) => e.kind === "agent" && e.agent.name === t.starterAgent!.name,
      );
      if (existing && existing.kind === "agent") {
        setAgentId(existing.id);
      }
    }
    setStep("agent");
  };

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
          mcp: mcpName(),
          eventFilter: {},
          pollIntervalSeconds: pollInterval(),
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
          promptTemplate: promptTemplate().trim() ? promptTemplate() : null,
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

      <details class="template-picker">
        <summary>Start from a template</summary>
        <div class="template-grid">
          <For each={TEMPLATES}>
            {(t) => (
              <button class="template-card" onClick={() => applyTemplate(t)}>
                <div class="template-card-meta">
                  <span class="badge badge-muted">{t.category}</span>
                  <span class="badge badge-muted">{t.trigger.kind}</span>
                  <Show when={t.guards.approvalGate}>
                    <span class="badge badge-pending">approval-gated</span>
                  </Show>
                </div>
                <h3>{t.name}</h3>
                <p>{t.description}</p>
                <Show when={t.requires.length > 0}>
                  <p class="muted small">
                    Requires:
                    <ul>
                      <For each={t.requires}>{(r) => <li>{r}</li>}</For>
                    </ul>
                  </p>
                </Show>
              </button>
            )}
          </For>
        </div>
      </details>

      <ol class="stepper">
        <li classList={{ active: step() === "agent", done: step() !== "agent" }}>1. Agent</li>
        <li
          classList={{
            active: step() === "trigger",
            done: step() === "prompt" || step() === "guards" || step() === "review",
          }}
        >
          2. Trigger
        </li>
        <li
          classList={{
            active: step() === "prompt",
            done: step() === "guards" || step() === "review",
          }}
        >
          3. Prompt
        </li>
        <li classList={{ active: step() === "guards", done: step() === "review" }}>4. Guards</li>
        <li classList={{ active: step() === "review" }}>5. Review</li>
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
            <button
              class="btn-primary"
              disabled={!agentId()}
              title={!agentId() ? "Pick an agent from the dropdown above first." : ""}
              onClick={() => setStep("trigger")}
            >
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "trigger"}>
        <section class="detail-block">
          <div class="provider-grid">
            <For each={["schedule", "webhook", "event", "manual"] as TriggerKind[]}>
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
                        : t === "event"
                          ? "Poll an MCP server every N seconds for new items."
                          : "Run manually from the UI."}
                  </p>
                </button>
              )}
            </For>
          </div>

          <Show when={triggerKind() === "schedule"}>
            <div class="settings-row">
              <label>Quick presets</label>
              <div class="chip-row">
                <For
                  each={[
                    ["* * * * *", "every minute"],
                    ["*/10 * * * *", "every 10 min"],
                    ["0 * * * *", "hourly"],
                    ["0 9 * * 1-5", "9am weekdays"],
                    ["0 9 * * 1", "Monday 9am"],
                    ["0 0 * * *", "daily midnight"],
                  ]}
                >
                  {([expr, label]) => (
                    <button class="chip" onClick={() => setCronExpr(expr)}>
                      {label}
                    </button>
                  )}
                </For>
              </div>
            </div>
            <div class="settings-row">
              <label>Cron</label>
              <input value={cronExpr()} onInput={(e) => setCronExpr(e.currentTarget.value)} />
              <p class="muted small">Standard 5-field cron. Live: <code>{cronExpr()}</code></p>
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
              <div class="webhook-url-row">
                <code>http://localhost:9876/hook/{webhookPath()}</code>
                <button
                  class="btn-secondary"
                  onClick={() =>
                    navigator.clipboard.writeText(
                      `http://localhost:9876/hook/${webhookPath()}`,
                    )
                  }
                >
                  Copy URL
                </button>
              </div>
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

          <Show when={triggerKind() === "event"}>
            <div class="settings-row">
              <label>MCP server name</label>
              <input
                value={mcpName()}
                onInput={(e) => setMcpName(e.currentTarget.value)}
                placeholder="gmail-mcp"
              />
              <p class="muted small">
                Senda spawns this MCP and calls its first <code>list_*</code> /{" "}
                <code>search_*</code> tool every poll cycle.
              </p>
            </div>
            <div class="settings-row">
              <label>Poll interval (seconds)</label>
              <input
                type="number"
                min="15"
                value={pollInterval()}
                onInput={(e) => setPollInterval(Number(e.currentTarget.value))}
              />
            </div>
          </Show>

          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("agent")}>
              Back
            </button>
            <button class="btn-primary" onClick={() => setStep("prompt")}>
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "prompt"}>
        <section class="detail-block">
          <h2>Prompt template</h2>
          <p class="muted small">
            Sent to the agent on every firing. Leave empty to use the trigger's natural payload
            (webhook body / cron placeholder / MCP item JSON). Use <code>{"{event}"}</code> as a
            placeholder to embed the trigger payload inside your template.
          </p>
          <div class="settings-row">
            <textarea
              rows={8}
              class="prompt-input"
              value={promptTemplate()}
              onInput={(e) => setPromptTemplate(e.currentTarget.value)}
              placeholder={
                triggerKind() === "webhook"
                  ? "A new event arrived:\n{event}\n\nClassify it and assign an owner."
                  : triggerKind() === "schedule"
                    ? "Triage the backlog created since the last run."
                    : ""
              }
            />
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("trigger")}>
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
            <button class="btn-secondary" onClick={() => setStep("prompt")}>
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
                promptTemplate: promptTemplate().trim() ? promptTemplate() : null,
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
