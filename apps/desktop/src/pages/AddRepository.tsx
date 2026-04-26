import { createSignal, Show, type Component } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";

import { refetchCatalog } from "../stores/catalog";

type Provider = "github" | "azure" | "generic";

type Step = "provider" | "repo" | "auth" | "confirm";

interface DeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

interface AddRepoArgs {
  url: string;
  authKind: "none" | "pat" | "oauth";
  authToken: string | null;
  branch: string | null;
  autoSync: boolean;
}

const detectProvider = (url: string): Provider => {
  try {
    const u = new URL(url);
    if (u.hostname === "github.com") return "github";
    if (u.hostname === "dev.azure.com" || u.hostname.endsWith(".visualstudio.com")) return "azure";
  } catch {
    // fall through
  }
  return "generic";
};

const AddRepository: Component = () => {
  const navigate = useNavigate();
  const [step, setStep] = createSignal<Step>("provider");
  const [provider, setProvider] = createSignal<Provider>("github");
  const [url, setUrl] = createSignal("");
  const [branch, setBranch] = createSignal("main");
  const [autoSync, setAutoSync] = createSignal(true);
  const [authKind, setAuthKind] = createSignal<"none" | "pat" | "oauth">("oauth");
  const [token, setToken] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [oauth, setOauth] = createSignal<DeviceCode | null>(null);

  const startDeviceFlow = async () => {
    setError(null);
    try {
      const code = await invoke<DeviceCode>("github_device_authorize", {});
      setOauth(code);
      pollDeviceFlow(code);
    } catch (e) {
      setError(`OAuth failed: ${e}`);
    }
  };

  const pollDeviceFlow = (code: DeviceCode) => {
    const interval = (code.interval + 1) * 1000;
    const handle = setInterval(async () => {
      try {
        const result = await invoke<{ accessToken: string } | null>("github_device_poll", {
          deviceCode: code.deviceCode,
        });
        if (result) {
          setToken(result.accessToken);
          setOauth(null);
          clearInterval(handle);
        }
      } catch (e) {
        clearInterval(handle);
        setError(String(e));
      }
    }, interval);
  };

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const args: AddRepoArgs = {
        url: url(),
        authKind: authKind(),
        authToken: authKind() === "none" ? null : token(),
        branch: branch().trim() ? branch() : null,
        autoSync: autoSync(),
      };
      await invoke("add_repo", { args });
      await refetchCatalog();
      navigate("/");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <h1>Add repository</h1>
        <p class="page-subtitle">
          Connect a Git repository so its agents appear in your catalog. Tokens are stored in the OS
          keychain — never in plain text on disk.
        </p>
      </header>

      <ol class="stepper">
        <li classList={{ active: step() === "provider", done: step() !== "provider" }}>
          1. Provider
        </li>
        <li
          classList={{
            active: step() === "repo",
            done: step() === "auth" || step() === "confirm",
          }}
        >
          2. Repository
        </li>
        <li classList={{ active: step() === "auth", done: step() === "confirm" }}>
          3. Authentication
        </li>
        <li classList={{ active: step() === "confirm" }}>4. Confirm</li>
      </ol>

      <Show when={step() === "provider"}>
        <section class="detail-block">
          <h2>Choose provider</h2>
          <div class="provider-grid">
            <ProviderCard
              name="GitHub"
              description="Public or private GitHub repos. OAuth Device Flow or PAT."
              active={provider() === "github"}
              onClick={() => setProvider("github")}
            />
            <ProviderCard
              name="Azure Repos"
              description="dev.azure.com or *.visualstudio.com — PAT only."
              active={provider() === "azure"}
              onClick={() => {
                setProvider("azure");
                setAuthKind("pat");
              }}
            />
            <ProviderCard
              name="Generic Git"
              description="GitLab, Bitbucket, self-hosted — clone/pull only, no PR API."
              active={provider() === "generic"}
              onClick={() => {
                setProvider("generic");
                setAuthKind("none");
              }}
            />
          </div>
          <div class="step-actions">
            <button class="btn-primary" onClick={() => setStep("repo")}>
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "repo"}>
        <section class="detail-block">
          <h2>Repository details</h2>
          <div class="settings-row">
            <label>Repository URL</label>
            <input
              type="url"
              value={url()}
              onInput={(e) => {
                setUrl(e.currentTarget.value);
                setProvider(detectProvider(e.currentTarget.value));
              }}
              placeholder="https://github.com/your-org/your-repo"
            />
          </div>
          <div class="settings-row">
            <label>Branch</label>
            <input
              type="text"
              value={branch()}
              onInput={(e) => setBranch(e.currentTarget.value)}
            />
          </div>
          <div class="settings-row">
            <label>
              <input
                type="checkbox"
                checked={autoSync()}
                onChange={(e) => setAutoSync(e.currentTarget.checked)}
              />{" "}
              Sync automatically (every 10 minutes)
            </label>
          </div>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("provider")}>
              Back
            </button>
            <button class="btn-primary" disabled={!url().trim()} onClick={() => setStep("auth")}>
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "auth"}>
        <section class="detail-block">
          <h2>Authentication</h2>
          <Show when={provider() === "github"}>
            <div class="auth-tabs">
              <button
                class="catalog-tab"
                classList={{ active: authKind() === "oauth" }}
                onClick={() => setAuthKind("oauth")}
              >
                Device Flow
              </button>
              <button
                class="catalog-tab"
                classList={{ active: authKind() === "pat" }}
                onClick={() => setAuthKind("pat")}
              >
                Personal access token
              </button>
              <button
                class="catalog-tab"
                classList={{ active: authKind() === "none" }}
                onClick={() => setAuthKind("none")}
              >
                None (public)
              </button>
            </div>
          </Show>

          <Show when={provider() === "azure"}>
            <p class="muted small">Azure Repos requires a Personal Access Token.</p>
          </Show>

          <Show when={provider() === "generic"}>
            <div class="auth-tabs">
              <button
                class="catalog-tab"
                classList={{ active: authKind() === "none" }}
                onClick={() => setAuthKind("none")}
              >
                None (public)
              </button>
              <button
                class="catalog-tab"
                classList={{ active: authKind() === "pat" }}
                onClick={() => setAuthKind("pat")}
              >
                Username + token
              </button>
            </div>
          </Show>

          <Show when={authKind() === "pat"}>
            <div class="settings-row">
              <label>Personal access token</label>
              <input
                type="password"
                value={token()}
                onInput={(e) => setToken(e.currentTarget.value)}
                placeholder="ghp_… or ATATT…"
              />
            </div>
          </Show>

          <Show when={authKind() === "oauth" && provider() === "github"}>
            <div class="oauth-flow">
              <Show
                when={oauth()}
                fallback={
                  <button class="btn-primary" onClick={startDeviceFlow}>
                    Start GitHub Device Flow
                  </button>
                }
              >
                {(_) => (
                  <div>
                    <p class="muted small">
                      Visit{" "}
                      <a href={oauth()!.verificationUri} target="_blank" rel="noopener noreferrer">
                        {oauth()!.verificationUri}
                      </a>{" "}
                      and enter:
                    </p>
                    <pre class="user-code">{oauth()!.userCode}</pre>
                    <p class="muted small">
                      Senda is polling every {oauth()!.interval}s. The button below activates once
                      authorization completes.
                    </p>
                  </div>
                )}
              </Show>
              <Show when={token()}>
                <p class="status status--succeeded">Authorized.</p>
              </Show>
            </div>
          </Show>

          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("repo")}>
              Back
            </button>
            <button
              class="btn-primary"
              disabled={
                authKind() !== "none" && !token().trim()
              }
              onClick={() => setStep("confirm")}
            >
              Continue
            </button>
          </div>
        </section>
      </Show>

      <Show when={step() === "confirm"}>
        <section class="detail-block">
          <h2>Confirm</h2>
          <dl class="confirm-list">
            <dt>Provider</dt>
            <dd>{provider()}</dd>
            <dt>URL</dt>
            <dd>
              <code>{url()}</code>
            </dd>
            <dt>Branch</dt>
            <dd>{branch()}</dd>
            <dt>Auth</dt>
            <dd>{authKind()}</dd>
            <dt>Auto-sync</dt>
            <dd>{autoSync() ? "yes" : "no"}</dd>
          </dl>
          <Show when={error()}>
            <p class="error-banner">{error()}</p>
          </Show>
          <div class="step-actions">
            <button class="btn-secondary" onClick={() => setStep("auth")} disabled={busy()}>
              Back
            </button>
            <button class="btn-primary" onClick={submit} disabled={busy()}>
              {busy() ? "Cloning…" : "Add repository"}
            </button>
          </div>
        </section>
      </Show>
    </section>
  );
};

interface ProviderCardProps {
  name: string;
  description: string;
  active: boolean;
  onClick: () => void;
}

const ProviderCard: Component<ProviderCardProps> = (props) => (
  <button
    type="button"
    class="provider-card"
    classList={{ active: props.active }}
    onClick={props.onClick}
  >
    <h3>{props.name}</h3>
    <p>{props.description}</p>
  </button>
);

export default AddRepository;
