import { createMemo, createSignal, For, onMount, Show, type Component } from "solid-js";
import { useNavigate, useSearchParams } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import type { AgentCli } from "senda-shared-types";

import { refetchCatalog } from "../stores/catalog";

interface Warning {
  target: AgentCli;
  fieldPath: string;
  message: string;
}

interface SaveResult {
  canonicalPath: string;
  generated: string[];
  warnings: Warning[];
}

const ALL_CLIS: AgentCli[] = ["copilot", "claude-code", "gemini"];

const DEFAULT_TEMPLATE = `---
name: my-agent
description: Replace this with what your agent does.
targets: [copilot]
tools: []
---

# My agent

Write your prompt here.
`;

const AgentEditor: Component = () => {
  const navigate = useNavigate();
  const [search] = useSearchParams<{ path?: string; draft?: string; template?: string }>();
  const [source, setSource] = createSignal<string>(DEFAULT_TEMPLATE);
  const [previousName, setPreviousName] = createSignal<string | null>(null);
  const [warnings, setWarnings] = createSignal<Warning[]>([]);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Form-mirrored fields kept in sync with the textarea.
  const [name, setName] = createSignal("my-agent");
  const [description, setDescription] = createSignal("");
  const [targets, setTargets] = createSignal<AgentCli[]>(["copilot"]);
  const [tools, setTools] = createSignal<string[]>([]);
  const [body, setBody] = createSignal("Write your prompt here.\n");

  onMount(async () => {
    const path = search.path;
    const tmpl = search.template;
    if (path) {
      try {
        const text = await invoke<string>("read_agent_source", { canonicalPath: path });
        setSource(text);
        applyParsedFromSource(text);
        setPreviousName(parseName(text));
      } catch (e) {
        setError(`Could not load: ${e}`);
      }
    } else if (tmpl) {
      setSource(tmpl);
      applyParsedFromSource(tmpl);
    }
  });

  const isDraft = () => search.draft === "1";

  const applyParsedFromSource = (text: string) => {
    const parsed = parseFrontmatter(text);
    if (parsed.name) setName(parsed.name);
    if (parsed.description !== undefined) setDescription(parsed.description);
    if (parsed.targets) setTargets(parsed.targets);
    if (parsed.tools) setTools(parsed.tools);
    setBody(parsed.body);
  };

  const buildSource = createMemo(() => {
    const targetsList = targets()
      .map((t) => `  - ${t}`)
      .join("\n");
    const toolsList = tools().length === 0 ? "[]" : `[${tools().join(", ")}]`;
    return `---
name: ${name()}
description: ${escapeYaml(description())}
targets:
${targetsList || "  - copilot"}
tools: ${toolsList}
---

${body()}`;
  });

  const toggleTarget = (cli: AgentCli) => {
    setTargets((prev) => (prev.includes(cli) ? prev.filter((c) => c !== cli) : [...prev, cli]));
  };

  const save = async () => {
    if (targets().length === 0) {
      setError("Pick at least one target CLI.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const result = await invoke<SaveResult>("save_agent", {
        args: {
          previousName: previousName(),
          source: source() === DEFAULT_TEMPLATE ? buildSource() : source(),
          draft: isDraft(),
        },
      });
      setWarnings(result.warnings);
      setPreviousName(name());
      await refetchCatalog();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const discard = () => navigate(-1);

  return (
    <section class="agent-editor">
      <header class="page-header">
        <h1>{previousName() ? `Edit ${previousName()}` : "New agent"}</h1>
        <p class="page-subtitle">
          The form on the left and the source on the right always reflect the same canonical
          document. Save writes <code>~/.senda/{isDraft() ? "drafts" : "agents"}/&lt;name&gt;.agent.md</code>{" "}
          and regenerates each target's native file.
        </p>
      </header>

      <Show when={warnings().length > 0}>
        <div class="warning-banner">
          <strong>Transpilation warnings</strong>
          <ul>
            <For each={warnings()}>
              {(w) => (
                <li>
                  <code>{w.fieldPath}</code> on {w.target}: {w.message}
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>

      <Show when={error()}>
        <div class="error-banner">{error()}</div>
      </Show>

      <div class="editor-grid">
        <section class="editor-form">
          <div class="settings-row">
            <label>Targets (required, min 1)</label>
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
                    onClick={() => toggleTarget(cli)}
                  >
                    {cli}
                  </button>
                )}
              </For>
            </div>
          </div>
          <div class="settings-row">
            <label>Name</label>
            <input value={name()} onInput={(e) => setName(e.currentTarget.value)} />
          </div>
          <div class="settings-row">
            <label>Description</label>
            <textarea
              rows={3}
              value={description()}
              onInput={(e) => setDescription(e.currentTarget.value)}
            />
          </div>
          <div class="settings-row">
            <label>Tools (comma-separated)</label>
            <input
              value={tools().join(", ")}
              onInput={(e) =>
                setTools(
                  e.currentTarget.value
                    .split(",")
                    .map((t) => t.trim())
                    .filter(Boolean),
                )
              }
            />
          </div>
          <div class="settings-row">
            <label>Prompt body</label>
            <textarea
              rows={10}
              value={body()}
              onInput={(e) => setBody(e.currentTarget.value)}
              class="prompt-input"
            />
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={discard}>
              Discard
            </button>
            <button
              class="btn-primary"
              onClick={save}
              disabled={saving() || targets().length === 0}
            >
              {saving() ? "Saving…" : "Save"}
            </button>
          </div>
        </section>
        <section class="editor-source">
          <label class="muted small">Canonical source</label>
          <textarea
            class="source-textarea"
            value={source() === DEFAULT_TEMPLATE ? buildSource() : source()}
            onInput={(e) => {
              setSource(e.currentTarget.value);
              applyParsedFromSource(e.currentTarget.value);
            }}
            spellcheck={false}
          />
        </section>
      </div>
    </section>
  );
};

// ── helpers ─────────────────────────────────────────────────────────────────

interface ParsedFrontmatter {
  name?: string;
  description?: string;
  targets?: AgentCli[];
  tools?: string[];
  body: string;
}

function parseFrontmatter(text: string): ParsedFrontmatter {
  const match = text.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
  if (!match) return { body: text };
  const fm = match[1];
  const body = match[2].replace(/^\n/, "");
  const result: ParsedFrontmatter = { body };

  for (const line of fm.split("\n")) {
    const m = line.match(/^([a-z-]+):\s*(.*)$/i);
    if (!m) continue;
    const key = m[1];
    const value = m[2];
    if (key === "name") result.name = value.trim();
    else if (key === "description") result.description = value.trim();
    else if (key === "targets") {
      const arrayMatch = value.match(/\[(.*)\]/);
      if (arrayMatch) {
        result.targets = arrayMatch[1]
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean) as AgentCli[];
      }
    } else if (key === "tools") {
      const arrayMatch = value.match(/\[(.*)\]/);
      if (arrayMatch) {
        result.tools = arrayMatch[1]
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean);
      }
    }
  }
  return result;
}

function parseName(text: string): string | null {
  return parseFrontmatter(text).name ?? null;
}

function escapeYaml(value: string): string {
  if (/[:\n]/.test(value)) {
    return JSON.stringify(value);
  }
  return value;
}

export default AgentEditor;
