import type { Component } from "solid-js";

const Sidebar: Component = () => {
  return (
    <aside class="sidebar">
      <header class="sidebar-header">
        <span class="brand">Senda</span>
        <span class="brand-tag">la senda de tus agentes</span>
      </header>

      <nav class="sidebar-nav">
        <span class="nav-section">PERSONAL</span>
        <a class="nav-link active" href="/">
          Catalog
        </a>
        <a class="nav-link disabled" href="#">
          Drafts
        </a>
        <a class="nav-link disabled" href="#">
          History
        </a>

        <span class="nav-section">CONNECTED REPOS</span>
        <span class="nav-empty">No repositories yet.</span>

        <span class="nav-section">AUTOMATIONS</span>
        <a class="nav-link disabled" href="#">
          Scheduler
        </a>
      </nav>

      <footer class="sidebar-footer">no account · local</footer>
    </aside>
  );
};

export default Sidebar;
