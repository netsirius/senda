import type { Component } from "solid-js";
import { A, useLocation } from "@solidjs/router";

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
        <A class="nav-link" classList={{ active: isActive("/history") }} href="/history">
          History
        </A>

        <span class="nav-section">CONNECTED REPOS</span>
        <span class="nav-empty">No repositories yet.</span>

        <span class="nav-section">AUTOMATIONS</span>
        <span class="nav-empty">Phase 3.</span>

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
