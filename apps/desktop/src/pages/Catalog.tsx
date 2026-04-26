import { createResource, For, Show, type Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";

type Greeting = { agentName: string; agentVersion: string };

async function fetchHello(): Promise<Greeting> {
  // The `hello_world` Tauri command is a Phase 0 smoke test that the IPC
  // boundary is alive. It will be replaced by `read_catalog` in Phase 1.
  return await invoke<Greeting>("hello_world", { name: "Hector" });
}

const Catalog: Component = () => {
  const [hello] = createResource(fetchHello);

  return (
    <section class="catalog">
      <header class="page-header">
        <h1>Catalog</h1>
        <p class="page-subtitle">Phase 0 — IPC smoke test.</p>
      </header>

      <div class="empty-state">
        <Show
          when={hello()}
          fallback={
            <p class="muted">
              Connecting to backend… (run <code>pnpm tauri dev</code> from <code>apps/desktop</code>)
            </p>
          }
        >
          {(payload) => (
            <div class="hello-card">
              <h2>Backend reachable</h2>
              <p>
                <strong>{payload().agentName}</strong> v{payload().agentVersion}
              </p>
              <ul class="checklist">
                <For each={[
                  "Cargo workspace builds",
                  "Tauri spawns the webview",
                  "IPC command roundtrips JSON",
                ]}>
                  {(item) => <li>✓ {item}</li>}
                </For>
              </ul>
            </div>
          )}
        </Show>
      </div>
    </section>
  );
};

export default Catalog;
