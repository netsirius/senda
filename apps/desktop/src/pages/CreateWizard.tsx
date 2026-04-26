import { createSignal, For, onCleanup, onMount, Show, type Component } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentCli } from "senda-shared-types";

interface Template {
  id: string;
  name: string;
  description: string;
  source: string;
  category?: string;
  /** MCPs that should be installed for this template's tools to work. */
  mcps?: string[];
}

interface DetectedAgent {
  originalPath: string;
  canonicalPath: string;
  cli: AgentCli;
  name: string;
}

interface GenerateResult {
  canonicalSource: string;
  rawOutput: string;
}

const TEMPLATES: Template[] = [
  // ── Generic (no MCP needed) ────────────────────────────────────────────
  simpleTemplate(
    "code-reviewer",
    "Code reviewer",
    "Reviews diffs and suggests improvements.",
    "Generic",
    "Review the following diff and propose concrete improvements. Be terse.",
  ),
  simpleTemplate(
    "pr-summarizer",
    "PR summarizer",
    "Generates a one-paragraph summary of a pull request.",
    "Generic",
    "Summarize this pull request in three bullet points.",
  ),
  simpleTemplate(
    "documentation-writer",
    "Docs writer",
    "Writes user-facing documentation for a feature.",
    "Generic",
    "Write clear documentation for the following feature.",
  ),
  simpleTemplate(
    "test-generator",
    "Test generator",
    "Generates unit tests for a target file.",
    "Generic",
    "Generate unit tests covering the listed cases.",
  ),
  simpleTemplate(
    "changelog-generator",
    "Changelog generator",
    "Builds a CHANGELOG entry from recent commits.",
    "Generic",
    "Group the commits by category and produce a CHANGELOG entry.",
  ),
  simpleTemplate(
    "security-auditor",
    "Security auditor",
    "Looks for OWASP-style issues in a code change.",
    "Generic",
    "Audit the diff for security issues. Report findings with severity.",
  ),

  // ── MCP-aware (need an MCP installed) ──────────────────────────────────
  mcpTemplate({
    id: "jira-triage-agent",
    name: "Jira triager",
    description: "Triage tickets de Jira con prioridad y squad. Necesita mcp/atlassian.",
    category: "Jira",
    mcps: ["atlassian"],
    tools: [
      "jira/search_issues",
      "jira/get_issue",
      "jira/transition_issue",
      "jira/add_comment",
      "jira/assign_issue",
    ],
    body: `Eres el triager de tickets para nuestro Jira.

## Constantes (sustituye a tu setup)

- **Project key**: PLAT
- **Workflow**: Backlog → To Refine → Ready → In Progress → Code Review → Done
- **Squads**: Platform, Billing, Growth, SRE

## Reglas

1. Solo tickets en estado **Backlog** creados últimas 4h.
2. Si menciona "down/p0/production" → priority=Blocker, transition a "To Refine", asignar a Platform squad.
3. Si menciona "billing/stripe/invoice" → squad Billing.
4. Si parece feature request sin criterio de aceptación → comentar pidiendo: customer impact + caso de uso concreto.
5. Si dudas → no toques, comenta "/cc @hsantos triage manual".

## Output

Lista qué hiciste:
- TICKET-1234 → priority=Blocker, squad=Platform
- TICKET-1235 → comentado, no asignado (ambiguo)
`,
  }),
  mcpTemplate({
    id: "confluence-publisher",
    name: "Confluence publisher",
    description: "Publica/actualiza páginas en Confluence. Para usar como step 2 de un chain.",
    category: "Confluence",
    mcps: ["atlassian"],
    tools: [
      "confluence/search",
      "confluence/get_page",
      "confluence/create_page",
      "confluence/update_page",
    ],
    body: `Eres el publicador de Confluence.

Recibes en {event} un Markdown listo para publicar (típicamente desde otro
agente que generó el contenido).

## Constantes

- **Space**: ENG
- **Página destino**: el primer header (#) del Markdown será el título.

## Tarea

1. Extrae el título del primer #.
2. \`confluence/search\` por título exacto en el space.
3. Si existe → \`confluence/update_page\` (replace).
4. Si no → \`confluence/create_page\` bajo el space.

## Output

Devuelve el link de la página final.
`,
  }),
  mcpTemplate({
    id: "linear-incident-creator",
    name: "Linear incident creator",
    description: "Crea incidents en Linear. Útil como step 2 cuando el primero detecta P0.",
    category: "Linear",
    mcps: ["linear"],
    tools: ["linear/create_issue", "linear/list_issues"],
    body: `Eres el creador de incidents de Linear.

Recibes en {event} la descripción de un problema (típicamente desde otro
agente que detectó un fallo).

## Constantes

- **Team**: TEAM_PLATFORM
- **Label**: incident-from-senda

## Tarea

1. Comprueba si ya existe issue similar (\`linear/list_issues\` filtrando por
   label "incident-from-senda" abiertos last 24h).
2. Si NO existe:
   \`linear/create_issue\`:
     team: TEAM_PLATFORM
     title: extrae headline del payload
     description: el payload completo
     priority: 1 (Urgent)
     labels: ["incident-from-senda"]
3. Si existe → no dupliques, devuelve "duplicate of LIN-XXX".
`,
  }),
  mcpTemplate({
    id: "slack-notifier",
    name: "Slack notifier",
    description: "Postea en Slack. Step final típico para notificar al equipo.",
    category: "Slack",
    mcps: ["slack"],
    tools: ["slack/post_message"],
    body: `Eres el notificador de Slack.

Recibes en {event} un mensaje listo para postear.

## Constantes

- **Canal por defecto**: #platform-alerts

## Tarea

1. Si el payload incluye un canal específico al inicio (formato \`channel:#xxx\\n…\`),
   usa ese canal. Si no, usa #platform-alerts.
2. \`slack/post_message\` con el mensaje formateado en mrkdwn.

## Output

Devuelve el ts del mensaje y el channel id.
`,
  }),
  mcpTemplate({
    id: "github-pr-reviewer",
    name: "GitHub PR reviewer",
    description: "Primera pasada de revisión de PR. Detecta riesgos sin aprobar/mergear.",
    category: "GitHub",
    mcps: ["github"],
    tools: [
      "github/get_pull_request",
      "github/get_pull_request_files",
      "github/add_pr_comment",
    ],
    body: `Eres el revisor de PRs de primera pasada.

Recibes en {event} un payload con el PR (típicamente del webhook de GitHub).

## Tarea

1. \`github/get_pull_request\` para detalles.
2. \`github/get_pull_request_files\` para el diff por fichero.
3. Analiza:
   - **Riesgos**: migrations, public API changes, security-sensitive paths,
     missing tests
   - **TL;DR** del cambio
4. \`github/add_pr_comment\` con un comentario top-level que incluya:
     ## TL;DR
     <párrafo>
     ## Risks
     - <riesgo 1>
     - <riesgo 2>

## Reglas

- Si no hay nada notable, comenta solo el TL;DR.
- NO apruebes ni mergees el PR — solo comentas.
`,
  }),
  mcpTemplate({
    id: "postgres-enricher",
    name: "Postgres enricher",
    description: "Enriquece tickets/issues con datos de tu DB. Solo SELECT.",
    category: "Data",
    mcps: ["postgres"],
    tools: ["postgres/query"],
    body: `Eres el enriquecedor con datos de Postgres.

Recibes en {event} un identificador (ticket id, customer email, etc).

## Constantes

- Solo SELECT — nunca UPDATE / DELETE / INSERT.
- Schema relevante: tabla \`customers\`, \`subscriptions\`, \`tickets_meta\`.

## Tarea

1. Identifica qué entidad menciona el payload.
2. \`postgres/query\` con SELECT para obtener:
   - tier (free / pro / enterprise)
   - mrr
   - signup_date
   - account_owner_email
3. Devuelve los datos en JSON estructurado para que el siguiente agente
   los use.
`,
  }),

  // ── Meta ───────────────────────────────────────────────────────────────
  simpleTemplate(
    "agent-creator",
    "Agent creator (meta)",
    "Helps you author other Senda agents.",
    "Generic",
    "Help me write a canonical Senda agent for the use case I describe.",
  ),
];

function simpleTemplate(
  id: string,
  name: string,
  description: string,
  category: string,
  body: string,
): Template {
  return {
    id,
    name,
    description,
    category,
    source: `---
name: ${id}
description: ${description}
targets: [copilot]
tools: []
---

# ${name}

${body}
`,
  };
}

interface McpTemplateOptions {
  id: string;
  name: string;
  description: string;
  category: string;
  mcps: string[];
  tools: string[];
  body: string;
}

function mcpTemplate(opts: McpTemplateOptions): Template {
  const tools = opts.tools.length > 0 ? `[${opts.tools.join(", ")}]` : "[]";
  return {
    id: opts.id,
    name: opts.name,
    description: opts.description,
    category: opts.category,
    mcps: opts.mcps,
    source: `---
name: ${opts.id}
description: ${opts.description}
targets: [copilot]
tools: ${tools}
---

${opts.body}`,
  };
}

const ALL_CLIS: AgentCli[] = ["copilot", "claude-code", "gemini"];

type Mode = "blank" | "template" | "generate" | "chat";

const CreateWizard: Component = () => {
  const navigate = useNavigate();
  const [step, setStep] = createSignal<"targets" | "mode" | "template" | "generate" | "chat">(
    "targets",
  );
  const [targets, setTargets] = createSignal<AgentCli[]>(["copilot"]);
  // `mode` lives only as a transient label for the active step — the step
  // itself drives what's rendered, so we just write to setMode without
  // reading it.
  const [, setMode] = createSignal<Mode | null>(null);
  const [intent, setIntent] = createSignal("");
  const [primaryCli, setPrimaryCli] = createSignal<AgentCli>("copilot");
  const [generating, setGenerating] = createSignal(false);
  const [generateError, setGenerateError] = createSignal<string | null>(null);
  const [detected, setDetected] = createSignal<DetectedAgent[]>([]);

  let unlisten: UnlistenFn | undefined;

  onMount(async () => {
    unlisten = await listen<DetectedAgent>("agents:detected", (e) => {
      setDetected((prev) => [e.payload, ...prev]);
    });
  });
  onCleanup(() => unlisten?.());

  const toggle = (cli: AgentCli) => {
    setTargets((prev) => (prev.includes(cli) ? prev.filter((c) => c !== cli) : [...prev, cli]));
  };

  const startBlank = () => {
    const blank = `---
name: new-agent
description: Replace this with what your agent does.
targets: [${targets().join(", ")}]
tools: []
---

# New agent

Write your prompt here.
`;
    navigate(`/agent/edit?template=${encodeURIComponent(blank)}`);
  };

  const startFromTemplate = (t: Template) => {
    const adapted = t.source.replace(/targets: \[copilot\]/, `targets: [${targets().join(", ")}]`);
    navigate(`/agent/edit?template=${encodeURIComponent(adapted)}`);
  };

  const generate = async () => {
    if (!intent().trim()) return;
    setGenerating(true);
    setGenerateError(null);
    try {
      const result = await invoke<GenerateResult>("generate_agent", {
        args: {
          primaryCli: primaryCli(),
          userIntent: intent(),
          targets: targets(),
        },
      });
      navigate(`/agent/edit?template=${encodeURIComponent(result.canonicalSource)}`);
    } catch (e) {
      setGenerateError(String(e));
    } finally {
      setGenerating(false);
    }
  };

  const adoptDetected = (d: DetectedAgent) => {
    navigate(`/agent/edit?path=${encodeURIComponent(d.canonicalPath)}&draft=1`);
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <h1>Create agent</h1>
        <p class="page-subtitle">Pick targets, then pick how to start.</p>
      </header>

      <ol class="stepper">
        <li classList={{ active: step() === "targets", done: step() !== "targets" }}>
          1. Targets
        </li>
        <li classList={{ active: step() === "mode" }}>2. Mode</li>
      </ol>

      <Show when={step() === "targets"}>
        <section class="detail-block">
          <h2>Which CLIs?</h2>
          <p class="muted small">Pick at least one. Senda will transpile to each.</p>
          <div class="chip-row">
            <For each={ALL_CLIS}>
              {(cli) => (
                <button
                  type="button"
                  class="chip"
                  classList={{
                    active: targets().includes(cli),
                    "chip-copilot": cli === "copilot",
                    "chip-claude": cli === "claude-code",
                    "chip-gemini": cli === "gemini",
                  }}
                  onClick={() => toggle(cli)}
                >
                  {cli}
                </button>
              )}
            </For>
          </div>
          <div class="step-actions">
            <button
              class="btn-primary"
              onClick={() => setStep("mode")}
              disabled={targets().length === 0}
            >
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "mode"}>
        <section class="detail-block">
          <h2>Mode</h2>
          <div class="provider-grid">
            <button class="provider-card" onClick={startBlank}>
              <h3>Blank</h3>
              <p>Start with a tiny canonical document and edit from scratch.</p>
            </button>
            <button class="provider-card" onClick={() => { setMode("template"); setStep("template"); }}>
              <h3>From template</h3>
              <p>{TEMPLATES.length} curated starting points across common agent shapes.</p>
            </button>
            <button class="provider-card" onClick={() => { setMode("generate"); setStep("generate"); }}>
              <h3>Generate from prompt</h3>
              <p>Describe what you want. Senda asks your primary CLI to draft the agent doc.</p>
            </button>
            <button class="provider-card" onClick={() => { setMode("chat"); setStep("chat"); }}>
              <h3>Chat with your CLI</h3>
              <p>
                Run your CLI's <code>agent-creator</code> in a separate window — Senda watches the
                native folders and imports any new agent it sees.
              </p>
            </button>
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("targets")}>
              Back
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "template"}>
        <section class="detail-block">
          <h2>Templates de agente</h2>
          <p class="muted small">
            Click sobre un template y se abre directamente en el editor con el body pre-rellenado.
            Para combinaciones más complejas (cron + chain + variables) ve a{" "}
            <a
              href="#"
              onClick={(e) => {
                e.preventDefault();
                navigate("/automations/new");
              }}
            >
              /automations/new
            </a>{" "}
            que tiene 12 templates de automation con bodies más ricos.
          </p>
          <div class="agent-grid">
            <For each={TEMPLATES}>
              {(t) => (
                <article
                  class="agent-card template-card-clickable"
                  onClick={() => startFromTemplate(t)}
                  style={{ cursor: "pointer" }}
                  title={`Click to open in editor: ${t.name}`}
                >
                  <header class="agent-card-header">
                    <h3>{t.name}</h3>
                    <Show when={t.category}>
                      <span class="badge badge-muted">{t.category}</span>
                    </Show>
                  </header>
                  <p class="agent-card-desc">{t.description}</p>
                  <Show when={t.mcps && t.mcps.length > 0}>
                    <p class="muted small">
                      <strong>Needs MCP:</strong>{" "}
                      <For each={t.mcps}>
                        {(m, i) => (
                          <>
                            <code>{m}</code>
                            {i() < t.mcps!.length - 1 ? ", " : ""}
                          </>
                        )}
                      </For>
                    </p>
                  </Show>
                </article>
              )}
            </For>
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("mode")}>
              Back
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "generate"}>
        <section class="detail-block">
          <h2>Generate from prompt</h2>
          <p class="muted small">
            Senda invokes your selected CLI without a custom agent and asks it to produce a Senda
            canonical document. The result lands in the editor — review before saving.
          </p>
          <div class="settings-row">
            <label>Primary CLI</label>
            <select
              value={primaryCli()}
              onChange={(e) => setPrimaryCli(e.currentTarget.value as AgentCli)}
            >
              <For each={ALL_CLIS}>{(c) => <option value={c}>{c}</option>}</For>
            </select>
          </div>
          <div class="settings-row">
            <label>What should the agent do?</label>
            <textarea
              rows={6}
              class="prompt-input"
              value={intent()}
              onInput={(e) => setIntent(e.currentTarget.value)}
              placeholder="Triage incoming Linear tickets every 10 minutes, classify by team, …"
            />
          </div>
          <Show when={generateError()}>
            <p class="error-banner">{generateError()}</p>
          </Show>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("mode")}>
              Back
            </button>
            <button
              class="btn-primary"
              disabled={generating() || !intent().trim()}
              onClick={generate}
            >
              {generating() ? "Asking your CLI…" : "Generate"}
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "chat"}>
        <section class="detail-block">
          <h2>Chat with your CLI's agent-creator</h2>
          <p class="muted small">
            Open your CLI in any terminal and run its agent creation flow (e.g.{" "}
            <code>copilot --agent=agent-creator</code>). When it writes a new file under one of the
            native folders, Senda detects it, converts to canonical, and lists it below — click to
            open in the editor.
          </p>
          <Show
            when={detected().length > 0}
            fallback={<p class="muted">Watching for new agent files…</p>}
          >
            <ul class="env-list">
              <For each={detected()}>
                {(d) => (
                  <li>
                    <button class="btn-secondary" onClick={() => adoptDetected(d)}>
                      Open <code>{d.name}</code> ({d.cli})
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("mode")}>
              Back
            </button>
          </div>
        </section>
      </Show>
    </section>
  );
};

export default CreateWizard;
