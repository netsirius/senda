import { For, Show, type Component } from "solid-js";
import { A, useLocation } from "@solidjs/router";
import { repos } from "../stores/repos";
import { count as pendingCount } from "../stores/approvals";

const Sidebar: Component = () => {
  const location = useLocation();
  const isActive = (path: string) => location.pathname === path;

  return (
    <aside class="sidebar">
      <header class="sidebar-header">
        <span class="brand">Senda</span>
        <span class="brand-tag">la senda de tus agentes</span>
      </header>

      <nav class="sidebar-nav">
        <span class="nav-section">PERSONAL</span>
        <A class="nav-link" classList={{ active: isActive("/") }} href="/">
          Catalog
        </A>
        <A class="nav-link" classList={{ active: isActive("/mcps") }} href="/mcps">
          MCP servers
        </A>
        <A class="nav-link" classList={{ active: isActive("/skills") }} href="/skills">
          Skills
        </A>
        <A class="nav-link" classList={{ active: isActive("/history") }} href="/history">
          History
        </A>

        <div class="nav-section-row">
          <span class="nav-section">CONNECTED REPOS</span>
          <A class="nav-section-action" href="/repos/add" title="Add repository">
            +
          </A>
        </div>
        <Show
          when={(repos() ?? []).length > 0}
          fallback={<span class="nav-empty">No repositories yet.</span>}
        >
          <For each={repos()}>
            {(repo) => (
              <A class="nav-link nav-link-repo" href={`/repos/${repo.id}`}>
                <span
                  class="repo-dot"
                  classList={{
                    "repo-dot--ok": !repo.lastSyncError,
                    "repo-dot--err": !!repo.lastSyncError,
                  }}
                />
                <span class="repo-label">{repo.repo}</span>
              </A>
            )}
          </For>
        </Show>

        <span class="nav-section">AUTOMATIONS</span>
        <A class="nav-link" classList={{ active: isActive("/automations") }} href="/automations">
          Scheduler
        </A>
        <A class="nav-link" classList={{ active: isActive("/approvals") }} href="/approvals">
          Pending
          <Show when={(pendingCount() ?? 0) > 0}>
            <span class="badge badge-pending">{pendingCount()}</span>
          </Show>
        </A>
        <A
          class="nav-link"
          classList={{ active: isActive("/automations/new") }}
          href="/automations/new"
        >
          + New automation
        </A>

        <div class="sidebar-grow" />

        <A class="nav-link" classList={{ active: isActive("/settings") }} href="/settings">
          Settings
        </A>
      </nav>

      <footer class="sidebar-footer">no account · local</footer>
    </aside>
  );
};

export default Sidebar;
