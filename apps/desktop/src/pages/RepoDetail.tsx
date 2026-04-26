import { createMemo, Show, type Component } from "solid-js";
import { A, useNavigate, useParams } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";

import { repos, refetchRepos } from "../stores/repos";
import { refetchCatalog } from "../stores/catalog";

const RepoDetail: Component = () => {
  const navigate = useNavigate();
  const params = useParams<{ id: string }>();
  const repo = createMemo(() => (repos() ?? []).find((r) => r.id === Number(params.id)));

  const syncNow = async () => {
    if (!repo()) return;
    try {
      await invoke("sync_repo", { id: repo()!.id });
      await refetchCatalog();
      await refetchRepos();
    } catch (e) {
      alert(`Sync failed: ${e}`);
    }
  };

  const disconnect = async () => {
    if (!repo()) return;
    if (!confirm(`Disconnect ${repo()!.url}? This removes the local cache; the remote is untouched.`)) {
      return;
    }
    await invoke("disconnect_repo", { id: repo()!.id });
    await refetchRepos();
    await refetchCatalog();
    navigate("/");
  };

  return (
    <Show
      when={repo()}
      fallback={
        <section class="catalog">
          <header class="page-header">
            <h1>Repository not found</h1>
            <A href="/">← Back</A>
          </header>
        </section>
      }
    >
      {(_) => (
        <section class="agent-detail">
          <nav class="breadcrumbs">
            <A href="/">Catalog</A>
            <span class="dot">›</span>
            <span>{repo()!.repo}</span>
          </nav>

          <header class="page-header agent-detail-header">
            <div>
              <h1>{repo()!.repo}</h1>
              <p class="page-subtitle">
                <code>{repo()!.url}</code>
              </p>
            </div>
            <div class="agent-detail-actions">
              <button class="btn-primary" onClick={syncNow}>
                Sync now
              </button>
              <button class="btn-secondary" onClick={disconnect}>
                Disconnect
              </button>
            </div>
          </header>

          <section class="detail-block">
            <h2>Status</h2>
            <dl class="confirm-list">
              <dt>Provider</dt>
              <dd>{repo()!.provider}</dd>
              <dt>Branch</dt>
              <dd>{repo()!.branch}</dd>
              <dt>Auth</dt>
              <dd>{repo()!.authKind}</dd>
              <dt>Auto-sync</dt>
              <dd>{repo()!.autoSync ? "yes" : "no"}</dd>
              <dt>Local path</dt>
              <dd>
                <code>{repo()!.localPath}</code>
              </dd>
              <dt>Last synced</dt>
              <dd>
                {repo()!.lastSyncedAt
                  ? new Date(repo()!.lastSyncedAt! * 1000).toLocaleString()
                  : "never"}
              </dd>
              <Show when={repo()!.lastSyncError}>
                <dt>Last error</dt>
                <dd class="status-error">{repo()!.lastSyncError}</dd>
              </Show>
            </dl>
          </section>
        </section>
      )}
    </Show>
  );
};

export default RepoDetail;
