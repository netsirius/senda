/* @refresh reload */
import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";

import App from "./App";
import Catalog from "./pages/Catalog";
import AgentDetail from "./pages/AgentDetail";
import AgentRunner from "./pages/AgentRunner";
import Settings from "./pages/Settings";
import History from "./pages/History";
import AddRepository from "./pages/AddRepository";
import RepoDetail from "./pages/RepoDetail";
import Automations from "./pages/Automations";
import CreateAutomation from "./pages/CreateAutomation";

import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root not found");
}

render(
  () => (
    <Router root={App}>
      <Route path="/" component={Catalog} />
      <Route path="/agent/run/*id" component={AgentRunner} />
      <Route path="/agent/*id" component={AgentDetail} />
      <Route path="/history" component={History} />
      <Route path="/settings" component={Settings} />
      <Route path="/repos/add" component={AddRepository} />
      <Route path="/repos/:id" component={RepoDetail} />
      <Route path="/automations" component={Automations} />
      <Route path="/automations/new" component={CreateAutomation} />
    </Router>
  ),
  root,
);
