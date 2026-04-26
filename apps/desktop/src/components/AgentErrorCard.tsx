import type { Component } from "solid-js";
import type { CatalogEntry } from "senda-shared-types";

interface Props {
  entry: Extract<CatalogEntry, { kind: "error" }>;
}

const AgentErrorCard: Component<Props> = (props) => {
  return (
    <article class="agent-card agent-card--error">
      <header class="agent-card-header">
        <h3>{props.entry.id}</h3>
        <span class="badge badge-error">parse error</span>
      </header>
      <p class="agent-card-desc agent-card-error-msg">{props.entry.message}</p>
      <footer class="agent-card-footer">
        <code class="agent-card-path">{props.entry.path}</code>
      </footer>
    </article>
  );
};

export default AgentErrorCard;
