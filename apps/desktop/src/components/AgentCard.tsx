import { For, Show, type Component } from "solid-js";
import type { Agent, AgentCli } from "senda-shared-types";

const CLI_LABEL: Record<AgentCli, string> = {
  copilot: "Copilot",
  "claude-code": "Claude",
  gemini: "Gemini",
};

const CLI_CLASS: Record<AgentCli, string> = {
  copilot: "chip chip-copilot",
  "claude-code": "chip chip-claude",
  gemini: "chip chip-gemini",
};

interface Props {
  agent: Agent;
}

const AgentCard: Component<Props> = (props) => {
  const isExternal = () => props.agent.source.kind === "external";
  const sourceLabel = () => {
    const s = props.agent.source;
    if (s.kind === "external") return `external · ${CLI_LABEL[s.originalCli]}`;
    if (s.kind === "personal") return "personal";
    return `repo · ${s.repoId}`;
  };

  return (
    <article class="agent-card" classList={{ "agent-card--external": isExternal() }}>
      <header class="agent-card-header">
        <h3>{props.agent.agent.name}</h3>
        <Show when={isExternal()}>
          <span class="badge badge-muted" title="Read-only — created outside of Senda.">
            external
          </span>
        </Show>
      </header>

      <p class="agent-card-desc">{props.agent.agent.description || "—"}</p>

      <ul class="chip-row">
        <For each={props.agent.agent.targets}>
          {(target) => <li class={CLI_CLASS[target]}>{CLI_LABEL[target]}</li>}
        </For>
      </ul>

      <footer class="agent-card-footer">
        <span>{props.agent.agent.tools.length} tools</span>
        <span class="dot">·</span>
        <span>{sourceLabel()}</span>
      </footer>
    </article>
  );
};

export default AgentCard;
