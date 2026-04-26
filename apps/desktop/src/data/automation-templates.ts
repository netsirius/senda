import type { AgentCli } from "senda-shared-types";

export type TriggerKind = "schedule" | "webhook" | "event" | "manual";

export interface AutomationTemplate {
  id: string;
  name: string;
  category: string;
  description: string;
  /** What this template needs in place to actually work. */
  requires: string[];
  trigger: {
    kind: TriggerKind;
    cron?: string;
    timezone?: string;
    path?: string;
    secret?: string;
    mcp?: string;
    pollIntervalSeconds?: number;
  };
  promptTemplate: string;
  guards: {
    idempotency: boolean;
    rateLimitPerHour: number;
    approvalGate: boolean;
    backpressure: "skip";
  };
  /** When the template is best paired with a fresh agent, this is the
   *  starter the user can drop into the editor. Targets is the suggested
   *  CLI list — leave empty if the user should choose. */
  starterAgent?: {
    name: string;
    description: string;
    targets: AgentCli[];
    tools: string[];
    body: string;
  };
}

const JIRA_BODY = `Eres un asistente de triage para Jira. Tu instancia y proyecto se
configuran fuera de este prompt — el MCP \`jira\` ya tiene acceso.

## Reglas de triage

1. Solo actúa sobre tickets que cumplan el filtro pasado en \`{event}\`.
2. Para cada ticket:
   - Clasifica severidad (P0/P1/P2/P3) por palabras clave
   - Sugiere asignación basada en componente / labels
3. Si dudas → no asignes, comenta pidiendo más datos.

## Output

Lista cada ticket en formato:
- TICKET-N → action, reason
`;

const LINEAR_BODY = `Eres un asistente para issues de Linear. El MCP \`linear\` está conectado.

## Tarea

Mira los issues nuevos del trigger payload y por cada uno:
- Asigna prioridad (Urgent/High/Medium/Low) por su descripción
- Si menciona "production" o "down" → Priority Urgent + label "incident"
- Si es feature request sin criterio de aceptación → comenta pidiéndolo

Resume cambios al final.`;

const PR_REVIEW_BODY = `You are reviewing a pull request that just opened. The webhook payload
contains the GitHub event in {event}.

## Output

- One-paragraph TL;DR of the change.
- Risk callouts: migrations, public API changes, security-sensitive
  paths, missing tests.
- Concrete suggestions, line-referenced when possible.

Be terse. Don't restate what the diff already shows.`;

const STANDUP_BODY = `Build a daily standup digest for the team.

Pull commits, PRs and Linear issues touched in the last 24 hours and
output a Markdown summary grouped by:
- 🚢 Shipped
- 🚧 In flight
- 🚨 Blocked

Keep names brief; one line per item.`;

const EMAIL_TRIAGE_BODY = `Inbound email arrived. Payload in {event}.

Classify into one of: customer-support / sales / spam / internal.
For customer-support, also estimate severity (P0..P3) and propose a
draft reply in the same language as the email.`;

export const TEMPLATES: AutomationTemplate[] = [
  {
    id: "jira-triage",
    name: "Triage Jira (every 15 min)",
    category: "Project management",
    description:
      "Cada 15 min revisa nuevos tickets del proyecto y asigna prioridad/squad.",
    requires: [
      "MCP `jira` añadido en /mcps",
      "Un agente que use jira/search_issues + jira/transition_issue",
    ],
    trigger: { kind: "schedule", cron: "*/15 * * * *", timezone: "UTC" },
    promptTemplate:
      "Triage tickets nuevos creados en los últimos 15 min en mi proyecto Jira. Si no hay novedades, responde 'no work' y termina.",
    guards: { idempotency: true, rateLimitPerHour: 60, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "jira-triage",
      description: "Triage de tickets nuevos del proyecto Jira.",
      targets: ["copilot"],
      tools: ["jira/search_issues", "jira/transition_issue", "jira/add_comment"],
      body: JIRA_BODY,
    },
  },
  {
    id: "jira-triage-with-approval",
    name: "Triage Jira con human-in-the-loop",
    category: "Project management",
    description:
      "Igual que el anterior pero pausa para aprobar cada acción en /approvals antes de ejecutar.",
    requires: [
      "MCP `jira` añadido",
      "Approval gate enabled (lo dejo activado en este template)",
    ],
    trigger: { kind: "schedule", cron: "*/30 * * * *", timezone: "UTC" },
    promptTemplate:
      "Analiza los tickets Jira nuevos y propón cambios. Lista cada acción explícitamente. Yo apruebo desde la UI.",
    guards: { idempotency: true, rateLimitPerHour: 30, approvalGate: true, backpressure: "skip" },
    starterAgent: {
      name: "jira-triage-cautious",
      description: "Propone cambios en Jira; nunca los aplica sin aprobación.",
      targets: ["copilot"],
      tools: ["jira/search_issues"],
      body: JIRA_BODY,
    },
  },
  {
    id: "linear-incident",
    name: "Linear: detectar incidentes (every 10 min)",
    category: "Project management",
    description: "Marca como Urgent + 'incident' issues que mencionen 'production' o 'down'.",
    requires: ["MCP `linear` añadido"],
    trigger: { kind: "schedule", cron: "*/10 * * * *", timezone: "UTC" },
    promptTemplate: "Busca issues creados en los últimos 10 min. Aplica reglas de severidad.",
    guards: { idempotency: true, rateLimitPerHour: 60, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "linear-incident-watcher",
      description: "Vigila Linear para escalar incidentes automáticamente.",
      targets: ["copilot"],
      tools: ["linear/list_issues", "linear/update_issue", "linear/add_label"],
      body: LINEAR_BODY,
    },
  },
  {
    id: "github-pr-review",
    name: "GitHub: revisión automática al abrir PR",
    category: "Engineering",
    description:
      "Webhook del repo dispara una revisión preliminar (TL;DR + riesgos) en cuanto abre un PR.",
    requires: [
      "Configurar webhook en GitHub apuntando a localhost:9876/hook/github-pr (necesita túnel: ngrok / cloudflared)",
      "MCP `github` con permisos de lectura",
    ],
    trigger: { kind: "webhook", path: "github-pr", secret: "" },
    promptTemplate: PR_REVIEW_BODY,
    guards: { idempotency: true, rateLimitPerHour: 200, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "pr-fast-review",
      description: "Primera pasada de revisión cuando abre un PR.",
      targets: ["claude-code"],
      tools: ["github/get_pull_request", "github/add_pr_comment"],
      body: PR_REVIEW_BODY,
    },
  },
  {
    id: "daily-standup",
    name: "Daily standup digest (cada día 8:30)",
    category: "Team ops",
    description:
      "Cada día a las 8:30 publica un resumen con shipped / in-flight / blocked basado en commits y issues.",
    requires: ["MCP `github`, `linear` o `jira` instalados"],
    trigger: { kind: "schedule", cron: "30 8 * * 1-5", timezone: "UTC" },
    promptTemplate:
      "Genera el digest del standup de hoy. Consolida actividad de las últimas 24 h.",
    guards: { idempotency: true, rateLimitPerHour: 5, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "team-standup-digest",
      description: "Resume actividad del equipo del último día.",
      targets: ["claude-code"],
      tools: ["github/list_commits", "github/list_pull_requests", "linear/list_issues"],
      body: STANDUP_BODY,
    },
  },
  {
    id: "email-triage-event",
    name: "Email triage (poll cada 5 min)",
    category: "Inbox",
    description: "Polea Gmail por correos nuevos y los clasifica.",
    requires: [
      "MCP `gmail` instalado y autenticado",
      "El MCP debe exponer un tool `list_*` con cursor `since`",
    ],
    trigger: { kind: "event", mcp: "gmail", pollIntervalSeconds: 300 },
    promptTemplate: EMAIL_TRIAGE_BODY,
    guards: { idempotency: true, rateLimitPerHour: 200, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "inbox-triage",
      description: "Triage de correos: clasificar y proponer respuesta.",
      targets: ["claude-code"],
      tools: ["gmail/search_messages", "gmail/draft_reply"],
      body: EMAIL_TRIAGE_BODY,
    },
  },
  {
    id: "ci-failure-handler",
    name: "Cuando CI falla → analiza log",
    category: "Engineering",
    description:
      "Webhook desde CI; el agente lee el log y crea un ticket si parece bug real (no flaky).",
    requires: [
      "Configurar webhook desde tu CI (Buildkite / GitHub Actions / etc) apuntando a localhost:9876/hook/ci-fail",
    ],
    trigger: { kind: "webhook", path: "ci-fail", secret: "" },
    promptTemplate:
      "Analiza el log de CI. Si parece un fallo reproducible (no flaky), crea un ticket con resumen y stack trace.",
    guards: { idempotency: true, rateLimitPerHour: 50, approvalGate: true, backpressure: "skip" },
    starterAgent: {
      name: "ci-failure-classifier",
      description: "Clasifica fallos de CI como flaky o reales.",
      targets: ["claude-code"],
      tools: ["github/get_workflow_run_logs"],
      body: PR_REVIEW_BODY,
    },
  },
  {
    id: "manual-investigation",
    name: "Investigación manual (Run now)",
    category: "Team ops",
    description:
      "Plantilla para automations que sólo se disparan a mano cuando necesitas investigar algo.",
    requires: [],
    trigger: { kind: "manual" },
    promptTemplate:
      "Investiga {event}. Devuelve hallazgos en bullets, evidencia incluida.",
    guards: { idempotency: false, rateLimitPerHour: 30, approvalGate: false, backpressure: "skip" },
  },
];
