import { ErrorBoundary, onCleanup, onMount, type ParentComponent } from "solid-js";
import { useNavigate } from "@solidjs/router";
import Sidebar from "./components/Sidebar";
import { refetchCatalog } from "./stores/catalog";

// Surface global runtime errors as a visible banner so the user isn't stuck
// staring at a frozen UI when a single store throws or a Tauri command
// rejects unexpectedly.
const reportGlobalError = (event: ErrorEvent | PromiseRejectionEvent) => {
  const message =
    "reason" in event ? String(event.reason) : (event as ErrorEvent).message;
  const banner = document.getElementById("senda-runtime-error");
  if (banner) {
    banner.textContent = `Runtime error: ${message}`;
    banner.style.display = "block";
  }
  console.error("[senda] global error", event);
};

const App: ParentComponent = (props) => {
  const navigate = useNavigate();

  onMount(() => {
    window.addEventListener("error", reportGlobalError);
    window.addEventListener("unhandledrejection", reportGlobalError);

    const handler = (e: KeyboardEvent) => {
      const meta = e.metaKey || e.ctrlKey;
      if (!meta) return;
      switch (e.key.toLowerCase()) {
        case "n":
          e.preventDefault();
          navigate("/create");
          break;
        case ",":
          e.preventDefault();
          navigate("/settings");
          break;
        case "r":
          if (e.shiftKey) return;
          e.preventDefault();
          void refetchCatalog();
          break;
        case "h":
          e.preventDefault();
          navigate("/history");
          break;
      }
    };
    window.addEventListener("keydown", handler);
    onCleanup(() => {
      window.removeEventListener("keydown", handler);
      window.removeEventListener("error", reportGlobalError);
      window.removeEventListener("unhandledrejection", reportGlobalError);
    });
  });

  return (
    <div class="app-shell">
      <Sidebar />
      <main class="main-pane">
        <div id="senda-runtime-error" class="runtime-error-banner" />
        <ErrorBoundary
          fallback={(err, reset) => (
            <div class="error-banner" style="margin:24px">
              <strong>Render error:</strong>
              <pre style="white-space:pre-wrap; font-size:12px; margin-top:8px">
                {String(err?.stack ?? err)}
              </pre>
              <button class="btn-secondary" style="margin-top:8px" onClick={reset}>
                Reset
              </button>
            </div>
          )}
        >
          {props.children}
        </ErrorBoundary>
        <footer class="senda-footer">
          Senda · build <code>{import.meta.env.MODE}</code>
        </footer>
      </main>
    </div>
  );
};

export default App;
