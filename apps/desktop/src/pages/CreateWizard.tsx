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
  const [selected, setSelected] = createSignal<Template | null>(null);
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
