import { createSignal, For, Show, type Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";

import { skills, refetchSkills, type SkillEntry } from "../stores/discovery";

const Skills: Component = () => {
  const [showForm, setShowForm] = createSignal(false);

  const remove = async (s: SkillEntry) => {
    if (s.cli !== "claude-code") {
      alert("Only Claude Code skills are managed by Senda today.");
      return;
    }
    if (
      !confirm(
        `Delete skill "${s.name}"? This removes the entire folder at ${s.path} — undoable from outside the trash.`,
      )
    ) {
      return;
    }
    try {
      await invoke("delete_skill", { cli: s.cli, name: s.name });
      await refetchSkills();
    } catch (e) {
      alert(`Delete failed: ${e}`);
    }
  };

  const reveal = (path: string) => invoke("reveal_in_finder", { path });

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>Skills</h1>
            <p class="page-subtitle">
              Knowledge bundles your CLIs load on demand. Senda manages skills under{" "}
              <code>~/.claude/skills/</code>; Copilot exposes skills as part of agents and Gemini
              has no standardized skills folder yet.
            </p>
          </div>
          <div class="catalog-header-actions">
            <button class="btn-primary" onClick={() => setShowForm(!showForm())}>
              {showForm() ? "Cancel" : "Create skill"}
            </button>
            <button class="btn-secondary" onClick={() => refetchSkills()}>
              Refresh
            </button>
          </div>
        </div>
      </header>

      <Show when={showForm()}>
        <CreateSkillForm
          onDone={async () => {
            setShowForm(false);
            await refetchSkills();
          }}
        />
      </Show>

      <Show
        when={(skills() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <h2>No skills found</h2>
            <p class="muted">
              Create one above, or drop a folder containing <code>SKILL.md</code> into{" "}
              <code>~/.claude/skills/</code> manually.
            </p>
          </div>
        }
      >
        <div class="agent-grid">
          <For each={skills()}>
            {(s) => (
              <article class="agent-card">
                <header class="agent-card-header">
                  <h3>{s.name}</h3>
                  <span class="badge badge-muted">{s.cli}</span>
                </header>
                <p class="agent-card-desc">{s.description ?? "No description."}</p>
                <footer class="agent-card-footer agent-card-actions">
                  <button class="btn-secondary small" onClick={() => reveal(s.path)}>
                    📂 Open
                  </button>
                  <button class="btn-danger small" onClick={() => remove(s)}>
                    Delete
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

const CreateSkillForm: Component<{ onDone: () => void | Promise<void> }> = (props) => {
  const [name, setName] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [body, setBody] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async () => {
    if (!name().trim() || !description().trim()) {
      setError("Name and description are required.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await invoke("create_skill", {
        args: {
          cli: "claude-code",
          name: name().trim(),
          description: description().trim(),
          body: body().trim() || "Skill body goes here.",
        },
      });
      await props.onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section class="detail-block">
      <h2>Create Claude Code skill</h2>
      <p class="muted small">
        Writes <code>~/.claude/skills/&lt;name&gt;/SKILL.md</code> with frontmatter Senda can
        re-read.
      </p>
      <div class="settings-row">
        <label>Name (kebab-case)</label>
        <input
          value={name()}
          onInput={(e) => setName(e.currentTarget.value)}
          placeholder="data-export-policy"
        />
      </div>
      <div class="settings-row">
        <label>Description (one line)</label>
        <input
          value={description()}
          onInput={(e) => setDescription(e.currentTarget.value)}
          placeholder="How to export tenant data safely."
        />
      </div>
      <div class="settings-row">
        <label>Body (Markdown)</label>
        <textarea
          rows={10}
          class="prompt-input"
          value={body()}
          onInput={(e) => setBody(e.currentTarget.value)}
          placeholder="# When to apply&#10;&#10;Use when a tenant requests a data export…"
        />
      </div>
      <Show when={error()}>
        <p class="error-banner">{error()}</p>
      </Show>
      <div class="step-actions">
        <button class="btn-primary" onClick={submit} disabled={busy()}>
          {busy() ? "Creating…" : "Create"}
        </button>
      </div>
    </section>
  );
};

export default Skills;
