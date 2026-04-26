import { createSignal, For, Show, type Component } from "solid-js";
import { useNavigate } from "@solidjs/router";
import type { AgentCli } from "senda-shared-types";

interface Template {
  id: string;
  name: string;
  description: string;
  source: string;
}

const TEMPLATES: Template[] = [
  template(
    "code-reviewer",
    "Code reviewer",
    "Reviews diffs and suggests improvements.",
    "Review the following diff and propose concrete improvements. Be terse.",
  ),
  template(
    "pr-summarizer",
    "PR summarizer",
    "Generates a one-paragraph summary of a pull request.",
    "Summarize this pull request in three bullet points.",
  ),
  template(
    "triage-emails",
    "Email triage",
    "Classifies inbound emails and drafts a response.",
    "Classify the email and draft a reply.",
  ),
  template(
    "ticket-classifier",
    "Ticket classifier",
    "Tags an inbound ticket with a single category and priority.",
    "Pick a single category and priority for the ticket.",
  ),
  template(
    "documentation-writer",
    "Docs writer",
    "Writes user-facing documentation for a feature.",
    "Write clear documentation for the following feature.",
  ),
  template(
    "test-generator",
    "Test generator",
    "Generates unit tests for a target file.",
    "Generate unit tests covering the listed cases.",
  ),
  template(
    "changelog-generator",
    "Changelog generator",
    "Builds a CHANGELOG entry from recent commits.",
    "Group the commits by category and produce a CHANGELOG entry.",
  ),
  template(
    "security-auditor",
    "Security auditor",
    "Looks for OWASP-style issues in a code change.",
    "Audit the diff for security issues. Report findings with severity.",
  ),
  template(
    "agent-creator",
    "Agent creator (meta)",
    "Helps you author other Senda agents.",
    "Help me write a canonical Senda agent for the use case I describe.",
  ),
];

function template(id: string, name: string, description: string, body: string): Template {
  return {
    id,
    name,
    description,
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

const ALL_CLIS: AgentCli[] = ["copilot", "claude-code", "gemini"];

const CreateWizard: Component = () => {
  const navigate = useNavigate();
  const [step, setStep] = createSignal<"targets" | "mode" | "template">("targets");
  const [targets, setTargets] = createSignal<AgentCli[]>(["copilot"]);
  const [selected, setSelected] = createSignal<Template | null>(null);

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

  return (
    <section class="catalog">
      <header class="page-header">
        <h1>Create agent</h1>
        <p class="page-subtitle">Pick targets, then choose how to start.</p>
      </header>

      <ol class="stepper">
        <li classList={{ active: step() === "targets", done: step() !== "targets" }}>
          1. Targets
        </li>
        <li classList={{ active: step() === "mode", done: step() === "template" }}>2. Mode</li>
        <li classList={{ active: step() === "template" }}>3. Template (optional)</li>
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
            <button class="provider-card" onClick={() => setStep("template")}>
              <h3>From template</h3>
              <p>{TEMPLATES.length} curated starting points across common agent shapes.</p>
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
          <h2>Templates</h2>
          <div class="agent-grid">
            <For each={TEMPLATES}>
              {(t) => (
                <article
                  class="agent-card"
                  classList={{ active: selected()?.id === t.id }}
                  onClick={() => setSelected(t)}
                  style={{ cursor: "pointer" }}
                >
                  <header class="agent-card-header">
                    <h3>{t.name}</h3>
                  </header>
                  <p class="agent-card-desc">{t.description}</p>
                </article>
              )}
            </For>
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("mode")}>
              Back
            </button>
            <button
              class="btn-primary"
              disabled={!selected()}
              onClick={() => selected() && startFromTemplate(selected()!)}
            >
              Use template
            </button>
          </div>
        </section>
      </Show>
    </section>
  );
};

export default CreateWizard;
