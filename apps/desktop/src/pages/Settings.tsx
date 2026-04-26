import { createSignal, type Component } from "solid-js";

const STORAGE_KEY = "senda.settings";

interface SettingsShape {
  copilotPath: string;
  claudePath: string;
  geminiPath: string;
  primaryCli: "copilot" | "claude-code" | "gemini";
}

function loadSettings(): SettingsShape {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...defaults(), ...JSON.parse(raw) };
  } catch {
    // fall through
  }
  return defaults();
}

function defaults(): SettingsShape {
  return {
    copilotPath: "copilot",
    claudePath: "claude",
    geminiPath: "gemini",
    primaryCli: "copilot",
  };
}

const Settings: Component = () => {
  const [settings, setSettings] = createSignal<SettingsShape>(loadSettings());
  const [tab, setTab] = createSignal<"general" | "cli-paths">("general");

  const update = <K extends keyof SettingsShape>(key: K, value: SettingsShape[K]) => {
    const next = { ...settings(), [key]: value };
    setSettings(next);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  };

  const reset = () => {
    localStorage.removeItem(STORAGE_KEY);
    setSettings(defaults());
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <h1>Settings</h1>
        <p class="page-subtitle">
          Local-only configuration. Settings are stored in <code>localStorage</code>.
        </p>

        <nav class="catalog-tabs" style="margin-top:12px">
          <button
            class="catalog-tab"
            classList={{ active: tab() === "general" }}
            onClick={() => setTab("general")}
          >
            General
          </button>
          <button
            class="catalog-tab"
            classList={{ active: tab() === "cli-paths" }}
            onClick={() => setTab("cli-paths")}
          >
            CLI paths
          </button>
        </nav>
      </header>

      {tab() === "general" ? (
        <section class="detail-block">
          <h2>Defaults</h2>
          <div class="settings-row">
            <label for="primary-cli">Primary CLI</label>
            <select
              id="primary-cli"
              value={settings().primaryCli}
              onChange={(e) =>
                update("primaryCli", e.currentTarget.value as SettingsShape["primaryCli"])
              }
            >
              <option value="copilot">Copilot</option>
              <option value="claude-code">Claude Code</option>
              <option value="gemini">Gemini</option>
            </select>
            <p class="muted small">
              Used as the default selection when a wizard or runner needs to pick a CLI.
            </p>
          </div>
        </section>
      ) : (
        <section class="detail-block">
          <h2>CLI paths</h2>
          <p class="muted small">
            Override the executable Senda spawns for each CLI. Auto-detected from{" "}
            <code>PATH</code> by default.
          </p>
          <div class="settings-row">
            <label>Copilot</label>
            <input
              type="text"
              value={settings().copilotPath}
              onInput={(e) => update("copilotPath", e.currentTarget.value)}
            />
          </div>
          <div class="settings-row">
            <label>Claude Code</label>
            <input
              type="text"
              value={settings().claudePath}
              onInput={(e) => update("claudePath", e.currentTarget.value)}
            />
          </div>
          <div class="settings-row">
            <label>Gemini</label>
            <input
              type="text"
              value={settings().geminiPath}
              onInput={(e) => update("geminiPath", e.currentTarget.value)}
            />
          </div>
        </section>
      )}

      <div class="settings-actions">
        <button class="btn-secondary" onClick={reset}>
          Reset to defaults
        </button>
      </div>
    </section>
  );
};

export default Settings;
