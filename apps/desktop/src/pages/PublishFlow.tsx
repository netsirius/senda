import { createMemo, createSignal, For, Show, type Component } from "solid-js";
import { A, useNavigate, useSearchParams } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";

import { repos } from "../stores/repos";

interface PublishResult {
  prUrl: string | null;
  prNumber: number | null;
  branch: string;
  manualPrRequired: boolean;
}

const PublishFlow: Component = () => {
  const navigate = useNavigate();
  const [search] = useSearchParams<{ name?: string }>();
  const agentName = () => search.name ?? "";

  const [repoId, setRepoId] = createSignal<number | null>(null);
  const [branchName, setBranchName] = createSignal(`senda/${agentName() || "draft"}`);
  const [title, setTitle] = createSignal(`Add ${agentName() || "agent"}`);
  const [body, setBody] = createSignal(buildPrBody(agentName() || "agent"));
  const [reviewers, setReviewers] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<PublishResult | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const writableRepos = createMemo(() => (repos() ?? []).filter((r) => r.authKind !== "none"));

  const submit = async () => {
    if (repoId() == null) return;
    setBusy(true);
    setError(null);
    try {
      const res = await invoke<PublishResult>("publish_agent", {
        args: {
          agentName: agentName(),
          repoId: repoId(),
          branchName: branchName(),
          title: title(),
          body: body(),
          reviewers: reviewers()
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean),
        },
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section class="catalog">
      <header class="page-header">
        <nav class="breadcrumbs">
          <A href="/">Catalog</A>
          <span class="dot">›</span>
          <span>Publish</span>
        </nav>
        <h1>Publish {agentName() || "agent"}</h1>
        <p class="page-subtitle">
          Push the canonical draft to a connected repo on a fresh branch and open a pull request.
        </p>
      </header>

      <Show when={!result()} fallback={<ResultPanel result={result()!} />}>
        <section class="detail-block">
          <div class="settings-row">
            <label>Destination repository</label>
            <select
              value={repoId() ?? ""}
              onChange={(e) => setRepoId(Number(e.currentTarget.value))}
            >
              <option value="" disabled>
                Pick a writable repo…
              </option>
              <For each={writableRepos()}>
                {(r) => (
                  <option value={r.id}>
                    {r.repo} — {r.url}
                  </option>
                )}
              </For>
            </select>
          </div>
          <div class="settings-row">
            <label>Branch name</label>
            <input value={branchName()} onInput={(e) => setBranchName(e.currentTarget.value)} />
          </div>
          <div class="settings-row">
            <label>PR title</label>
            <input value={title()} onInput={(e) => setTitle(e.currentTarget.value)} />
          </div>
          <div class="settings-row">
            <label>PR body</label>
            <textarea
              rows={10}
              value={body()}
              onInput={(e) => setBody(e.currentTarget.value)}
              class="prompt-input"
            />
          </div>
          <div class="settings-row">
            <label>Reviewers (comma-separated GitHub usernames)</label>
            <input value={reviewers()} onInput={(e) => setReviewers(e.currentTarget.value)} />
          </div>

          <Show when={error()}>
            <p class="error-banner">{error()}</p>
          </Show>

          <div class="step-actions">
            <button class="btn-secondary" onClick={() => navigate(-1)}>
              Cancel
            </button>
            <button
              class="btn-primary"
              disabled={busy() || repoId() == null || !agentName()}
              onClick={submit}
            >
              {busy() ? "Publishing…" : "Publish"}
            </button>
          </div>
        </section>
      </Show>
    </section>
  );
};

const ResultPanel: Component<{ result: PublishResult }> = (props) => (
  <section class="detail-block">
    <h2>Publish complete</h2>
    <Show
      when={!props.result.manualPrRequired}
      fallback={
        <p>
          The branch <code>{props.result.branch}</code> was pushed. Generic git providers don't have
          a standard PR API — open the PR in your provider's UI.
        </p>
      }
    >
      <p>
        PR <code>#{props.result.prNumber}</code> opened. View it at{" "}
        <a href={props.result.prUrl ?? "#"} target="_blank" rel="noopener noreferrer">
          {props.result.prUrl}
        </a>
        .
      </p>
    </Show>
    <A class="btn-primary" href="/">
      Back to catalog
    </A>
  </section>
);

function buildPrBody(name: string): string {
  return `## Summary

Adds the canonical Senda agent \`${name}\`.

## Review checklist

- [ ] Name follows kebab-case convention
- [ ] Description is clear and concise
- [ ] Tools listed are minimum necessary
- [ ] No hardcoded credentials in prompt
- [ ] Prompt fits within 30k chars

---
*Created by Senda*
`;
}

export default PublishFlow;
