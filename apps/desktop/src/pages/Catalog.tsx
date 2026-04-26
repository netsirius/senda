import { createMemo, createSignal, For, Show, type Component } from "solid-js";
import { A } from "@solidjs/router";

import AgentCard from "../components/AgentCard";
import AgentErrorCard from "../components/AgentErrorCard";
import { catalog as entries, refetchCatalog } from "../stores/catalog";

type Tab = "all" | "personal" | "external" | "errors";

const Catalog: Component = () => {
  const [search, setSearch] = createSignal("");
  const [tab, setTab] = createSignal<Tab>("all");

  const counts = createMemo(() => {
    const list = entries() ?? [];
    let personal = 0;
    let external = 0;
    let errors = 0;
    for (const e of list) {
      if (e.kind === "error") errors += 1;
      else if (e.source.kind === "personal") personal += 1;
      else if (e.source.kind === "external") external += 1;
    }
    return { all: list.length, personal, external, errors };
  });

  const filtered = createMemo(() => {
    const q = search().trim().toLowerCase();
    const t = tab();
    return (entries() ?? []).filter((e) => {
      if (t === "errors" && e.kind !== "error") return false;
      if (t !== "errors" && e.kind === "error") return false;
      if (t === "personal" && e.kind === "agent" && e.source.kind !== "personal") return false;
      if (t === "external" && e.kind === "agent" && e.source.kind !== "external") return false;
      if (!q) return true;
      if (e.kind === "error") {
        return e.id.toLowerCase().includes(q) || e.message.toLowerCase().includes(q);
      }
      return (
        e.agent.name.toLowerCase().includes(q) ||
        e.agent.description.toLowerCase().includes(q) ||
        e.agent.tools.some((tool) => tool.toLowerCase().includes(q))
      );
    });
  });

  return (
    <section class="catalog">
      <header class="page-header">
        <div class="page-title-row">
          <div>
            <h1>Catalog</h1>
            <p class="page-subtitle">
              {entries.loading
                ? "Scanning your agent folders…"
                : `${counts().all} agent${counts().all === 1 ? "" : "s"} found.`}
            </p>
          </div>
          <div class="catalog-header-actions">
            <A class="btn-primary" href="/create">
              New agent
            </A>
            <button class="btn-secondary" onClick={() => refetchCatalog()}>
              Refresh
            </button>
          </div>
        </div>

        <div class="catalog-controls">
          <input
            type="search"
            class="search-input"
            placeholder="Filter by name, description or tool"
            value={search()}
            onInput={(e) => setSearch(e.currentTarget.value)}
          />
          <nav class="catalog-tabs">
            <Tab name="all" active={tab() === "all"} count={counts().all} onSelect={setTab} label="All" />
            <Tab
              name="personal"
              active={tab() === "personal"}
              count={counts().personal}
              onSelect={setTab}
              label="Personal"
            />
            <Tab
              name="external"
              active={tab() === "external"}
              count={counts().external}
              onSelect={setTab}
              label="External"
            />
            <Tab
              name="errors"
              active={tab() === "errors"}
              count={counts().errors}
              onSelect={setTab}
              label="Errors"
            />
          </nav>
        </div>
      </header>

      <Show
        when={!entries.loading && filtered().length === 0}
        fallback={
          <div class="agent-grid">
            <For each={filtered()}>
              {(e) =>
                e.kind === "agent" ? (
                  <A class="agent-card-link" href={`/agent/${encodeURIComponent(e.id)}`}>
                    <AgentCard agent={e} />
                  </A>
                ) : (
                  <AgentErrorCard entry={e} />
                )
              }
            </For>
          </div>
        }
      >
        <EmptyState totalKnown={counts().all} />
      </Show>

      <Show when={entries.error}>
        <p class="error-banner">Failed to read catalog: {String(entries.error)}</p>
      </Show>
    </section>
  );
};

interface TabProps {
  name: Tab;
  label: string;
  active: boolean;
  count: number;
  onSelect: (t: Tab) => void;
}

const Tab: Component<TabProps> = (props) => (
  <button
    type="button"
    class="catalog-tab"
    classList={{ active: props.active }}
    onClick={() => props.onSelect(props.name)}
  >
    {props.label}
    <span class="catalog-tab-count">{props.count}</span>
  </button>
);

const EmptyState: Component<{ totalKnown: number }> = (props) => (
  <div class="empty-state">
    <Show
      when={props.totalKnown === 0}
      fallback={<p class="muted">No agents match the current filter.</p>}
    >
      <h2>No agents yet</h2>
      <p class="muted">
        Senda scans <code>~/.senda/agents/</code>, <code>~/.copilot/agents/</code>,{" "}
        <code>~/.claude/agents/</code> and <code>~/.gemini/agents/</code>.
      </p>
      <p class="muted">
        Drop a canonical <code>.agent.md</code> into <code>~/.senda/agents/</code>, or create one
        with your CLI of choice and Senda will pick it up.
      </p>
    </Show>
  </div>
);

export default Catalog;
