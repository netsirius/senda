import { createMemo, createResource, createSignal, For, Show, type Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { AgentCli } from "senda-shared-types";

import { mcps, refetchMcps, type InstalledMcp } from "../stores/discovery";
import { dockerArgsFor, MCP_CATALOG, type CatalogEntry } from "../data/mcp-catalog";

const CLI_LABEL: Record<AgentCli, string> = {
  copilot: "Copilot",
  "claude-code": "Claude Code",
  gemini: "Gemini",
};

const ALL_CLIS: AgentCli[] = ["copilot", "claude-code", "gemini"];

interface DockerStatus {
  installed: boolean;
  version: string | null;
}

const Mcps: Component = () => {
  const [mode, setMode] = createSignal<"none" | "catalog" | "manual">("none");
  const [presetEntry, setPresetEntry] = createSignal<CatalogEntry | null>(null);
  const [dockerStatus] = createResource(() => invoke<DockerStatus>("docker_status"));

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
            <button
              class="btn-primary"
              onClick={() => {
                setMode(mode() === "catalog" ? "none" : "catalog");
                setPresetEntry(null);
              }}
            >
              {mode() === "catalog" ? "Cancel" : "Browse Docker catalog"}
            </button>
            <button
              class="btn-secondary"
              onClick={() => {
                setMode(mode() === "manual" ? "none" : "manual");
                setPresetEntry(null);
              }}
            >
              {mode() === "manual" ? "Cancel" : "Add manually"}
            </button>
            <button class="btn-secondary" onClick={() => refetchMcps()}>
              Refresh
            </button>
          </div>
        </div>
      </header>

      <Show when={mode() === "catalog"}>
        <CatalogBrowser
          dockerStatus={dockerStatus()}
          onPick={(entry) => {
            setPresetEntry(entry);
            setMode("manual");
          }}
        />
      </Show>

      <Show when={mode() === "manual"}>
        <AddMcpForm
          preset={presetEntry()}
          onDone={async () => {
            setMode("none");
            setPresetEntry(null);
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
              Browse the Docker catalog above for one-click setup of common MCPs (Atlassian,
              GitHub, Linear, Slack, Postgres…), or add one manually.
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

const CatalogBrowser: Component<{
  dockerStatus: DockerStatus | undefined;
  onPick: (entry: CatalogEntry) => void;
}> = (props) => {
  return (
    <section class="detail-block">
      <h2>Docker MCP catalog</h2>
      <Show
        when={props.dockerStatus?.installed}
        fallback={
          <p class="warning-banner">
            Docker doesn't seem to be installed (or not in PATH). The catalog generates{" "}
            <code>docker run …</code> incantations — install Docker Desktop or pick "Add
            manually" if your MCPs are local binaries.
          </p>
        }
      >
        <p class="muted small">
          Docker {props.dockerStatus?.version ?? "(detected)"}. Click an entry to pre-fill the
          form below with the right docker run flags and required env vars.
        </p>
      </Show>
      <div class="template-grid">
        <For each={MCP_CATALOG}>
          {(entry) => (
            <button class="template-card" onClick={() => props.onPick(entry)}>
              <div class="template-card-meta">
                <span class="badge badge-muted">{entry.category}</span>
                <span class="badge badge-muted">{entry.image}</span>
              </div>
              <h3>{entry.name}</h3>
              <p>{entry.description}</p>
              <Show when={entry.env.length > 0}>
                <p class="muted small">
                  Needs:{" "}
                  <For each={entry.env.filter((v) => v.required)}>
                    {(v, i) => (
                      <>
                        <code>{v.key}</code>
                        {i() < entry.env.filter((x) => x.required).length - 1 ? ", " : ""}
                      </>
                    )}
                  </For>
                </p>
              </Show>
            </button>
          )}
        </For>
      </div>
    </section>
  );
};

const AddMcpForm: Component<{
  preset: CatalogEntry | null;
  onDone: () => void | Promise<void>;
}> = (props) => {
  const initialCli: AgentCli = "copilot";
  const [cli, setCli] = createSignal<AgentCli>(initialCli);
  const [name, setName] = createSignal(props.preset?.id ?? "");
  const [type, setType] = createSignal<"local" | "remote">("local");
  const [command, setCommand] = createSignal(props.preset ? "docker" : "");
  const [args, setArgs] = createSignal(
    props.preset ? dockerArgsFor(props.preset).join(" ") : "",
  );
  const [url, setUrl] = createSignal("");
  const [envValues, setEnvValues] = createSignal<Record<string, string>>({});
  const [envText, setEnvText] = createSignal("");
  const [pullBeforeSave, setPullBeforeSave] = createSignal(true);
  const [busy, setBusy] = createSignal(false);
  const [busyMessage, setBusyMessage] = createSignal("");
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
      // Optional pre-pull when adding from catalog.
      if (props.preset && pullBeforeSave()) {
        setBusyMessage(`Pulling ${props.preset.image}…`);
        try {
          await invoke("docker_pull_image", { image: props.preset.image });
        } catch (e) {
          // Don't block the save — the CLI will pull on first run.
          console.warn("docker pull failed:", e);
        }
      }
      setBusyMessage("Saving config…");

      const env: Record<string, string> = {};
      if (props.preset) {
        for (const v of props.preset.env) {
          const value = envValues()[v.key]?.trim();
          if (v.required && !value) {
            setError(`${v.label} is required.`);
            return;
          }
          if (value) env[v.key] = value;
        }
      } else {
        for (const line of envText().split("\n")) {
          const eq = line.indexOf("=");
          if (eq <= 0) continue;
          env[line.slice(0, eq).trim()] = line.slice(eq + 1).trim();
        }
      }

      const argv = type() === "local"
        ? args()
            .split(/\s+/)
            .filter(Boolean)
        : [];

      await invoke("add_mcp", {
        args: {
          cli: cli(),
          name: name().trim(),
          type: type(),
          command: type() === "local" ? command().trim() : null,
          args: argv,
          url: type() === "remote" ? url().trim() : null,
          env,
        },
      });
      await props.onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setBusyMessage("");
    }
  };

  return (
    <section class="detail-block">
      <h2>{props.preset ? `Configure ${props.preset.name}` : "Add MCP server"}</h2>
      <Show when={props.preset}>
        <p class="muted small">
          Docs: <a href={props.preset!.docs} target="_blank" rel="noopener noreferrer">{props.preset!.docs}</a>
        </p>
      </Show>

      <div class="settings-row">
        <label>Target CLI</label>
        <select value={cli()} onChange={(e) => setCli(e.currentTarget.value as AgentCli)}>
          <For each={ALL_CLIS}>{(c) => <option value={c}>{CLI_LABEL[c]}</option>}</For>
        </select>
      </div>
      <div class="settings-row">
        <label>Name</label>
        <input value={name()} onInput={(e) => setName(e.currentTarget.value)} placeholder="atlassian" />
      </div>

      <Show when={!props.preset}>
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
      </Show>

      <Show when={type() === "local"}>
        <div class="settings-row">
          <label>Command</label>
          <input
            value={command()}
            onInput={(e) => setCommand(e.currentTarget.value)}
            placeholder={props.preset ? "docker" : "gmail-mcp"}
          />
        </div>
        <div class="settings-row">
          <label>Args (space-separated)</label>
          <input
            value={args()}
            onInput={(e) => setArgs(e.currentTarget.value)}
            placeholder={props.preset ? "run -i --rm …" : "--read-only"}
          />
        </div>
      </Show>

      <Show when={type() === "remote" && !props.preset}>
        <div class="settings-row">
          <label>URL</label>
          <input
            value={url()}
            onInput={(e) => setUrl(e.currentTarget.value)}
            placeholder="https://example.com/mcp"
          />
        </div>
      </Show>

      <Show when={props.preset} fallback={
        <div class="settings-row">
          <label>Env (one KEY=value per line)</label>
          <textarea
            rows={3}
            value={envText()}
            onInput={(e) => setEnvText(e.currentTarget.value)}
            class="prompt-input"
          />
        </div>
      }>
        <div class="settings-row">
          <label>Required configuration</label>
          <div class="env-grid">
            <For each={props.preset!.env}>
              {(v) => (
                <div class="env-row">
                  <label class="env-row-label">
                    <code>{v.key}</code>
                    <span>{v.label}</span>
                    {v.required && <span class="env-required">required</span>}
                  </label>
                  <input
                    type={v.kind === "password" ? "password" : "text"}
                    placeholder={v.hint}
                    value={envValues()[v.key] ?? ""}
                    onInput={(e) =>
                      setEnvValues((prev) => ({ ...prev, [v.key]: e.currentTarget.value }))
                    }
                  />
                </div>
              )}
            </For>
          </div>
        </div>

        <div class="settings-row">
          <label>
            <input
              type="checkbox"
              checked={pullBeforeSave()}
              onChange={(e) => setPullBeforeSave(e.currentTarget.checked)}
            />{" "}
            Pre-pull <code>{props.preset!.image}</code> before save (recommended)
          </label>
        </div>
      </Show>

      <Show when={busyMessage()}>
        <p class="muted small">{busyMessage()}</p>
      </Show>
      <Show when={error()}>
        <p class="error-banner">{error()}</p>
      </Show>
      <div class="step-actions">
        <button class="btn-primary" onClick={submit} disabled={busy()}>
          {busy() ? "Working…" : "Save"}
        </button>
      </div>
    </section>
  );
};

export default Mcps;
