import { createMemo, createSignal, onCleanup, Show, type Component } from "solid-js";
import { A, useParams, useSearchParams } from "@solidjs/router";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import type { Agent, AgentCli } from "senda-shared-types";

import { catalog as entries } from "../stores/catalog";

interface RunAgentArgs {
  agentId: string;
  agentSource: string;
  cli: AgentCli;
  agentName: string;
  prompt: string;
  cwd: string | null;
  dryRun: boolean;
}

interface RunAgentResult {
  executionId: string;
}

type Status = "idle" | "running" | "succeeded" | "failed" | "cancelled";

const AgentRunner: Component = () => {
  const params = useParams();
  const [search] = useSearchParams<{ cli?: AgentCli }>();
  const [prompt, setPrompt] = createSignal("");
  const [status, setStatus] = createSignal<Status>("idle");
  const [executionId, setExecutionId] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  let termHost: HTMLDivElement | undefined;
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let unlistenOutput: UnlistenFn | undefined;
  let unlistenDone: UnlistenFn | undefined;

  const targetEntry = createMemo(() => {
    const id = decodeURIComponent(params.id ?? "");
    return (entries() ?? []).find((e) => e.kind === "agent" && e.id === id);
  });

  const agent = createMemo<Agent | null>(() => {
    const e = targetEntry();
    return e && e.kind === "agent" ? e : null;
  });

  const targetCli = createMemo<AgentCli | null>(() => {
    const a = agent();
    if (!a) return null;
    if (search.cli && a.agent.targets.includes(search.cli)) return search.cli;
    return a.agent.targets[0] ?? null;
  });

  const sourceLabel = createMemo(() => {
    const a = agent();
    if (!a) return "";
    if (a.source.kind === "personal") return "personal";
    if (a.source.kind === "external") return `external/${a.source.originalCli}`;
    return `repo/${a.source.repoId}`;
  });

  const writeChunk = (chunk: string) => {
    if (!term) return;
    term.write(chunk.replace(/\n/g, "\r\n"));
  };

  const ensureTerminal = () => {
    if (term || !termHost) return;
    term = new Terminal({
      convertEol: true,
      cursorBlink: false,
      fontFamily: 'ui-monospace, "SFMono-Regular", Menlo, monospace',
      fontSize: 12,
      theme: {
        background: "#0c0d10",
        foreground: "#e6e8ec",
        cursor: "#b388ff",
      },
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(termHost);
    fit.fit();
    const onResize = () => fit?.fit();
    window.addEventListener("resize", onResize);
    onCleanup(() => window.removeEventListener("resize", onResize));
  };

  const tearDownListeners = async () => {
    if (unlistenOutput) {
      unlistenOutput();
      unlistenOutput = undefined;
    }
    if (unlistenDone) {
      unlistenDone();
      unlistenDone = undefined;
    }
  };

  onCleanup(() => {
    void tearDownListeners();
    term?.dispose();
  });

  const run = async (dryRun: boolean) => {
    const a = agent();
    const cli = targetCli();
    if (!a || !cli) return;
    if (!prompt().trim()) {
      setError("Prompt cannot be empty.");
      return;
    }
    setError(null);
    ensureTerminal();
    term?.clear();
    term?.writeln(`[90m$ senda run ${a.agent.name} (${cli})${dryRun ? " [dry-run]" : ""}[0m`);

    setStatus("running");
    try {
      const args: RunAgentArgs = {
        agentId: a.id,
        agentSource: sourceLabel(),
        cli,
        agentName: a.agent.name,
        prompt: prompt(),
        cwd: null,
        dryRun,
      };
      const result = await invoke<RunAgentResult>("run_agent", { args });
      setExecutionId(result.executionId);

      unlistenOutput = await listen<{ chunk: string }>(
        `execution:${result.executionId}:output`,
        (e) => writeChunk(e.payload.chunk),
      );
      unlistenDone = await listen<{ exitCode: number | null; error: string | null }>(
        `execution:${result.executionId}:done`,
        (e) => {
          const code = e.payload.exitCode;
          if (e.payload.error) {
            setError(e.payload.error);
            setStatus("failed");
          } else if (code === 0) {
            setStatus("succeeded");
          } else {
            setStatus("failed");
          }
          term?.writeln(
            `\r\n[90m— exit ${code ?? "?"}${e.payload.error ? ` (${e.payload.error})` : ""}[0m`,
          );
          void tearDownListeners();
        },
      );
    } catch (e) {
      setStatus("failed");
      setError(String(e));
    }
  };

  const cancel = async () => {
    const id = executionId();
    if (!id) return;
    await invoke("cancel_execution", { executionId: id });
    setStatus("cancelled");
  };

  return (
    <Show
      when={agent()}
      fallback={
        <section class="catalog">
          <header class="page-header">
            <h1>Agent not found</h1>
            <A href="/">← Back to catalog</A>
          </header>
        </section>
      }
    >
      {(_) => (
        <section class="agent-runner">
          <nav class="breadcrumbs">
            <A href="/">Catalog</A>
            <span class="dot">›</span>
            <A href={`/agent/${encodeURIComponent(agent()!.id)}`}>{agent()!.agent.name}</A>
            <span class="dot">›</span>
            <span>Run</span>
          </nav>

          <header class="page-header runner-header">
            <div>
              <h1>Run {agent()!.agent.name}</h1>
              <p class="page-subtitle">
                Target: <code>{targetCli()}</code> · Source: <code>{sourceLabel()}</code>
              </p>
            </div>
            <div class="agent-detail-actions">
              <button
                class="btn-primary"
                onClick={() => run(false)}
                disabled={status() === "running" || !prompt().trim()}
              >
                Run
              </button>
              <button
                class="btn-secondary"
                onClick={() => run(true)}
                disabled={status() === "running" || !prompt().trim()}
                title="Sets SENDA_DRY_RUN=1 in the agent's environment."
              >
                Dry run
              </button>
              <button
                class="btn-secondary"
                onClick={cancel}
                disabled={status() !== "running"}
              >
                Cancel
              </button>
            </div>
          </header>

          <textarea
            class="prompt-input"
            placeholder="Type the prompt to send to the agent…"
            rows={4}
            value={prompt()}
            onInput={(e) => setPrompt(e.currentTarget.value)}
            disabled={status() === "running"}
          />

          <div class="status-row">
            <span class={`status status--${status()}`}>{status()}</span>
            <Show when={error()}>
              <span class="status-error">{error()}</span>
            </Show>
          </div>

          <div ref={(el) => (termHost = el)} class="xterm-host" />
        </section>
      )}
    </Show>
  );
};

export default AgentRunner;
