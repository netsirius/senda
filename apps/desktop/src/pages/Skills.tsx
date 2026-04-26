import { For, Show, type Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";

import { skills, refetchSkills, type SkillEntry } from "../stores/discovery";

const Skills: Component = () => {
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

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>Skills</h1>
            <p class="page-subtitle">
              Knowledge bundles your CLIs load on demand. Senda lists what's already on disk under{" "}
              <code>~/.claude/skills/</code>; Copilot exposes skills as part of agents and Gemini
              has no standardized skills folder yet.
            </p>
          </div>
          <button class="btn-secondary" onClick={() => refetchSkills()}>
            Refresh
          </button>
        </div>
      </header>

      <Show
        when={(skills() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <h2>No skills found</h2>
            <p class="muted">
              Drop a folder containing <code>SKILL.md</code> into <code>~/.claude/skills/</code>{" "}
              and it'll show up here.
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
                  <code class="agent-card-path">{s.path}</code>
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

export default Skills;
