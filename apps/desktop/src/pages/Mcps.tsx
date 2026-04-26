import { createMemo, createSignal, For, Show, type Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { AgentCli } from "senda-shared-types";

import { mcps, refetchMcps, type InstalledMcp } from "../stores/discovery";

const CLI_LABEL: Record<AgentCli, string> = {
  copilot: "Copilot",
  "claude-code": "Claude Code",
  gemini: "Gemini",
};

const ALL_CLIS: AgentCli[] = ["copilot", "claude-code", "gemini"];

const Mcps: Component = () => {
  const [showForm, setShowForm] = createSignal(false);

  const grouped = createMemo(() => {
    const by: Record<AgentCli, InstalledMcp[]> = {
      copilot: [],
      "claude-code": [],
      gemini: [],
    };
    for (const m of mcps() ?? []) {
      by[m.cli].push(m);
    }
    return by;
  });

  const remove = async (m: InstalledMcp) => {
    if (!confirm(`Delete MCP "${m.name}" from ${CLI_LABEL[m.cli]}'s config?`)) return;
    try {
      await invoke("delete_mcp", { cli: m.cli, name: m.name });
      await refetchMcps();
    } catch (e) {
      alert(`Delete failed: ${e}`);
    }
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>MCP servers</h1>
            <p class="page-subtitle">
              Read straight from each CLI's config:{" "}
              <code>~/.copilot/mcp-servers.json</code>,{" "}
              <code>~/.claude.json</code> / <code>~/.claude/settings.json</code>,{" "}
              <code>~/.gemini/settings.json</code>. Senda only writes the entry you add — other
              keys are preserved.
            </p>
          </div>
          <div class="catalog-header-actions">
            <button class="btn-primary" onClick={() => setShowForm(!showForm())}>
              {showForm() ? "Cancel" : "Add MCP"}
            </button>
            <button class="btn-secondary" onClick={() => refetchMcps()}>
              Refresh
            </button>
          </div>
        </div>
      </header>

      <Show when={showForm()}>
        <AddMcpForm
          onDone={async () => {
            setShowForm(false);
            await refetchMcps();
          }}
        />
      </Show>

      <Show
        when={(mcps() ?? []).length > 0}
        fallback={
          <div class="empty-state">
            <h2>No MCPs detected</h2>
            <p class="muted">
              Add one above, or configure it through your CLI directly (
              <code>copilot mcp add</code>, <code>claude mcp add</code>) and refresh.
            </p>
          </div>
        }
      >
        <For each={Object.entries(grouped()) as [AgentCli, InstalledMcp[]][]}>
          {([cli, items]) => (
            <Show when={items.length > 0}>
              <section class="detail-block">
                <h2>{CLI_LABEL[cli]}</h2>
                <table class="mcp-table">
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th>Type</th>
                      <th>Command / URL</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={items}>
                      {(m) => (
                        <tr>
                          <td>
                            <code>{m.name}</code>
                          </td>
                          <td>{m.type}</td>
                          <td>
                            <code>{m.command ?? m.url ?? "—"}</code>
                          </td>
                          <td>
                            <button class="btn-danger small" onClick={() => remove(m)}>
                              Delete
                            </button>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </section>
            </Show>
          )}
        </For>
      </Show>
    </section>
  );
};

const AddMcpForm: Component<{ onDone: () => void | Promise<void> }> = (props) => {
  const [cli, setCli] = createSignal<AgentCli>("copilot");
  const [name, setName] = createSignal("");
  const [type, setType] = createSignal<"local" | "remote">("local");
  const [command, setCommand] = createSignal("");
  const [args, setArgs] = createSignal("");
  const [url, setUrl] = createSignal("");
  const [envText, setEnvText] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async () => {
    if (!name().trim()) {
      setError("Name is required.");
      return;
    }
    if (type() === "local" && !command().trim()) {
      setError("Command is required for local MCPs.");
      return;
    }
    if (type() === "remote" && !url().trim()) {
      setError("URL is required for remote MCPs.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const env: Record<string, string> = {};
      for (const line of envText().split("\n")) {
        const eq = line.indexOf("=");
        if (eq <= 0) continue;
        env[line.slice(0, eq).trim()] = line.slice(eq + 1).trim();
      }
      await invoke("add_mcp", {
        args: {
          cli: cli(),
          name: name().trim(),
          type: type(),
          command: type() === "local" ? command().trim() : null,
          args:
            type() === "local"
              ? args()
                  .split(/\s+/)
                  .filter(Boolean)
              : [],
          url: type() === "remote" ? url().trim() : null,
          env,
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
      <h2>Add MCP server</h2>
      <div class="settings-row">
        <label>Target CLI</label>
        <select value={cli()} onChange={(e) => setCli(e.currentTarget.value as AgentCli)}>
          <For each={ALL_CLIS}>{(c) => <option value={c}>{CLI_LABEL[c]}</option>}</For>
        </select>
      </div>
      <div class="settings-row">
        <label>Name</label>
        <input value={name()} onInput={(e) => setName(e.currentTarget.value)} placeholder="gmail" />
      </div>
      <div class="settings-row">
        <label>Type</label>
        <select
          value={type()}
          onChange={(e) => setType(e.currentTarget.value as "local" | "remote")}
        >
          <option value="local">local (subprocess)</option>
          <option value="remote">remote (URL)</option>
        </select>
      </div>
      <Show when={type() === "local"}>
        <div class="settings-row">
          <label>Command</label>
          <input
            value={command()}
            onInput={(e) => setCommand(e.currentTarget.value)}
            placeholder="gmail-mcp"
          />
        </div>
        <div class="settings-row">
          <label>Args (space-separated)</label>
          <input
            value={args()}
            onInput={(e) => setArgs(e.currentTarget.value)}
            placeholder="--read-only"
          />
        </div>
      </Show>
      <Show when={type() === "remote"}>
        <div class="settings-row">
          <label>URL</label>
          <input
            value={url()}
            onInput={(e) => setUrl(e.currentTarget.value)}
            placeholder="https://example.com/mcp"
          />
        </div>
      </Show>
      <div class="settings-row">
        <label>Env (one KEY=value per line)</label>
        <textarea
          rows={3}
          value={envText()}
          onInput={(e) => setEnvText(e.currentTarget.value)}
          class="prompt-input"
        />
      </div>
      <Show when={error()}>
        <p class="error-banner">{error()}</p>
      </Show>
      <div class="step-actions">
        <button class="btn-primary" onClick={submit} disabled={busy()}>
          {busy() ? "Saving…" : "Save"}
        </button>
      </div>
    </section>
  );
};

export default Mcps;
