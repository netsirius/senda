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

  // ── Compound templates — multi-tool / multi-MCP orchestration ────────────
  // These exist to teach the pattern: ONE agent can chain reads + writes
  // across MCPs in a single LLM session. Pick the closest one, swap the
  // {{PLACEHOLDERS}} for your real values, and you have a working chain.

  {
    id: "weekly-jira-confluence-digest",
    name: "Weekly digest: Jira → Confluence",
    category: "Compound",
    description:
      "Cada lunes 9:00 lee los tickets cerrados la semana pasada, agrupa por componente y publica un resumen ejecutivo en Confluence.",
    requires: [
      "MCP `atlassian` añadido (cubre Jira + Confluence en uno)",
      "Página Confluence destino: 'Weekly Platform Update' en el space PLAT — créala vacía la primera vez",
    ],
    trigger: { kind: "schedule", cron: "0 9 * * 1", timezone: "UTC" },
    promptTemplate:
      "Genera el digest semanal del proyecto y publícalo en Confluence. Reemplaza la página existente.",
    guards: { idempotency: true, rateLimitPerHour: 5, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "weekly-platform-digest",
      description: "Resume actividad de Jira de la semana y la publica en Confluence.",
      targets: ["claude-code"],
      tools: [
        "jira/search_issues",
        "confluence/search",
        "confluence/get_page",
        "confluence/update_page",
        "confluence/create_page",
      ],
      body: `Eres el redactor del digest semanal del equipo Platform.

## Constantes (sustituye con los tuyos)

- **Project key**: PLAT
- **Confluence space**: PLAT
- **Página destino**: "Weekly Platform Update"
- **Componentes que importan**: Auth, Billing, API, SRE

## Tarea

1. \`jira/search_issues\` con JQL:
     project = PLAT AND status = Done AND resolved >= -7d
2. Agrupa los issues por **component** (campo \`components[0].name\`).
3. Para cada componente compón un párrafo (3-5 líneas) que destaque:
   - Cuántos tickets se cerraron
   - 2-3 highlights con su key (PLAT-1234)
   - Contributors únicos (assignee.displayName)
4. Construye un Markdown con esta estructura:
     # Weekly Platform Update — semana del {{Monday}} al {{Sunday}}
     ## Auth
     <párrafo>
     ## Billing
     <párrafo>
     ...
5. \`confluence/search\` la página "Weekly Platform Update" en space PLAT.
   - Si existe → \`confluence/update_page\` con el Markdown convertido a storage format.
   - Si no existe → \`confluence/create_page\` bajo el space PLAT como página raíz.
6. Devuelve el link de la página al final del output.

## Reglas

- Si no hubo movimiento en un componente, lo omites (no escribas "no hubo
  cambios", solo no incluyas la sección).
- Si Jira devuelve 0 issues, NO publiques en Confluence — devuelve solo:
  "No closed tickets last week, skipping digest."
- Sé terse. Bullets > prosa.
`,
    },
  },

  {
    id: "jira-p0-to-linear",
    name: "Cross-MCP: Jira P0 → Linear incident",
    category: "Compound",
    description:
      "Cuando aparece un P0 en Jira, crea automáticamente un issue de incidente en Linear y enlaza ambos.",
    requires: [
      "MCP `atlassian` (Jira)",
      "MCP `linear`",
      "Project key Jira y team_id Linear que tienes que sustituir en el body",
      "Approval gate ON: el template lo activa porque crear incidents en Linear es escritura cross-tool",
    ],
    trigger: { kind: "schedule", cron: "*/10 * * * *", timezone: "UTC" },
    promptTemplate:
      "Busca P0 nuevos en Jira y créalos como incidents en Linear si aún no están sincronizados.",
    guards: { idempotency: true, rateLimitPerHour: 30, approvalGate: true, backpressure: "skip" },
    starterAgent: {
      name: "jira-p0-bridge",
      description: "Sincroniza tickets P0 de Jira hacia Linear como incidents.",
      targets: ["claude-code"],
      tools: [
        "jira/search_issues",
        "jira/get_issue",
        "jira/add_comment",
        "linear/list_issues",
        "linear/create_issue",
      ],
      body: `Eres el puente entre Jira y Linear para incidents.

## Constantes

- **Jira project**: PLAT
- **Linear team_id**: TEAM_PLATFORM
- **Linear label**: incident-from-jira

## Tarea

1. \`jira/search_issues\` con JQL:
     project = PLAT AND priority = Blocker AND created >= -1h
2. Para cada issue:
   a. \`linear/list_issues\` filtrando por descripción que contenga el Jira key
      — esto es el chequeo de "¿ya existe en Linear?".
   b. Si NO existe:
      - \`jira/get_issue\` para extraer summary, description, reporter
      - \`linear/create_issue\` con:
          team: {{LINEAR_TEAM_ID}}
          title: "[{{JIRA_KEY}}] {{summary}}"
          description: incluye link a Jira + summary original
          priority: 1 (Urgent en Linear)
          labels: ["incident-from-jira"]
      - Anota el resultado.
   c. Si ya existe → skip.
3. Después de procesar todos, vuelve a Jira:
   - Por cada Jira sincronizado nuevo → \`jira/add_comment\`:
     "Mirrored to Linear: {{LINEAR_URL}}"

## Output

Lista qué creaste:
- PLAT-1234 → LINEAR-ENG-567 (created)
- PLAT-1235 → already in Linear (skipped)
`,
    },
  },

  {
    id: "stale-confluence-pages",
    name: "Compound: Confluence stale pages → Jira review tasks",
    category: "Compound",
    description:
      "Una vez al mes detecta páginas Confluence sin actualizar en 6 meses y crea un ticket Jira asignado al autor pidiendo review.",
    requires: [
      "MCP `atlassian` (Confluence + Jira)",
      "Project key Jira para los tickets de review",
    ],
    trigger: { kind: "schedule", cron: "0 9 1 * *", timezone: "UTC" },
    promptTemplate: "Detecta páginas obsoletas y crea tickets de review.",
    guards: { idempotency: true, rateLimitPerHour: 5, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "confluence-stale-detector",
      description: "Encuentra páginas Confluence viejas y abre tickets de review en Jira.",
      targets: ["claude-code"],
      tools: [
        "confluence/search",
        "confluence/get_page",
        "jira/search_issues",
        "jira/create_issue",
        "jira/assign_issue",
      ],
      body: `Eres el guardián de la documentación. Tu job es asegurar que las
páginas Confluence importantes se mantengan vigentes.

## Constantes

- **Confluence space**: ENG
- **Jira project (para tickets)**: DOCS
- **Umbral de "stale"**: 180 días sin updates
- **Excluir labels**: ["archived", "external", "playground"]

## Tarea

1. \`confluence/search\` con CQL:
     space = ENG AND lastModified < now("-180d") AND label != "archived"
2. Para cada página resultante:
   a. \`confluence/get_page\` para extraer last_editor.username
   b. Comprueba si ya hay ticket abierto:
      \`jira/search_issues\` con JQL:
        project = DOCS AND status != Done AND summary ~ "Review {{PAGE_TITLE}}"
   c. Si NO hay → \`jira/create_issue\`:
        project: DOCS
        type: Task
        summary: "Review stale page: {{PAGE_TITLE}}"
        description:
          "Page hasn't been updated since {{lastModified}}.
          {{PAGE_URL}}
          Either update with current state, archive, or close this ticket if
          intentional."
        priority: Low
   d. \`jira/assign_issue\` al last_editor (si tiene cuenta Jira;
      si no, déjalo unassigned y comenta "Original author not in Jira").

## Reglas

- Si una página tiene >1 año stale Y está en root del space → priority=Medium.
- Nunca crees más de 50 tickets en una ejecución (rate limit cortés con tu
  proyecto Jira).

## Output

Resume tickets creados con sus IDs.
`,
    },
  },

  {
    id: "support-webhook-triage",
    name: "Webhook: Support → Confluence FAQ + Linear",
    category: "Compound",
    description:
      "Cuando llega un ticket de soporte (webhook), busca en Confluence FAQ; si hay match, sugiere respuesta; si no, crea bug en Linear.",
    requires: [
      "MCP `atlassian` (Confluence)",
      "MCP `linear`",
      "Webhook desde tu sistema de soporte (Zendesk/Intercom/etc) apuntando a localhost:9876/hook/support — necesita túnel (ngrok) si la fuente es externa",
    ],
    trigger: { kind: "webhook", path: "support" },
    promptTemplate:
      "Triage del ticket de soporte recibido en {event}. Busca FAQ matching, crea bug si aplica.",
    guards: { idempotency: true, rateLimitPerHour: 200, approvalGate: false, backpressure: "skip" },
    starterAgent: {
      name: "support-triage-bridge",
      description: "Triage de tickets de soporte: busca FAQ y escala a Linear si es bug.",
      targets: ["claude-code"],
      tools: [
        "confluence/search",
        "confluence/get_page",
        "linear/create_issue",
      ],
      body: `Eres el primer triage de tickets de soporte.

## Payload

El webhook trae el ticket en {event} con esta forma esperada:
\`\`\`json
{
  "id": "TICK-1234",
  "subject": "...",
  "body": "...",
  "customer": { "email": "...", "tier": "free|pro|enterprise" }
}
\`\`\`

## Decisión

1. Extrae intent del subject + body. Categorías:
   - **how-to** (cómo hacer X): probablemente FAQ-able
   - **bug**: comportamiento inesperado, error message
   - **feature-request**: pedido nuevo
   - **billing**: stripe, factura, downgrade
   - **other**

2. Si **how-to**:
   - \`confluence/search\` en space DOCS con CQL: \`title ~ "{{terms}}"\`
   - Si encuentras match (≥80% confidence):
     - Output: "MATCH FAQ: <page_title> — <page_url>. Suggested reply: <2-paragraph rephrasing of the page in customer's tone>"
   - Si no:
     - Output: "No FAQ match. Suggested reply: <crafted from your knowledge>. Consider adding to FAQ."

3. Si **bug**:
   - \`linear/create_issue\`:
       team: TEAM_PRODUCT
       title: "[Support] {{ticket.subject}}"
       description: incluye email del cliente + body del ticket + reproducción si la hay
       priority: customer.tier === "enterprise" ? 1 (Urgent) : 2 (High)
       labels: ["from-support", "bug"]
   - Output: "Created Linear issue {{LINEAR_ID}}"

4. Si **feature-request**:
   - No abras ticket, solo responde con clasificación + suggested response:
     "Thanks for the suggestion! We track requests at <url>. Voted up."

5. Si **billing** o **other**:
   - Output: "Routing to human. Category: <X>"

## Reglas

- Tier "enterprise" siempre route a humano si dudas.
- Nunca cierres el ticket original (no tienes esos tools — solo sugerimos).
`,
    },
  },
];
