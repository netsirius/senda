import { onCleanup, onMount, type ParentComponent } from "solid-js";
import { useNavigate } from "@solidjs/router";
import Sidebar from "./components/Sidebar";
import { refetchCatalog } from "./stores/catalog";

const App: ParentComponent = (props) => {
  const navigate = useNavigate();

  onMount(() => {
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
          // Avoid stealing the OS-level reload combo when the user really
          // wants it (Cmd+Shift+R / Ctrl+Shift+R).
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
    onCleanup(() => window.removeEventListener("keydown", handler));
  });

  return (
    <div class="app-shell">
      <Sidebar />
      <main class="main-pane">{props.children}</main>
    </div>
  );
};

export default App;
