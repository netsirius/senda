import type { ParentComponent } from "solid-js";
import Sidebar from "./components/Sidebar";

const App: ParentComponent = (props) => {
  return (
    <div class="app-shell">
      <Sidebar />
      <main class="main-pane">{props.children}</main>
    </div>
  );
};

export default App;
