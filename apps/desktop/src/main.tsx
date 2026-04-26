/* @refresh reload */
import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";

import App from "./App";
import Catalog from "./pages/Catalog";

import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root not found");
}

render(
  () => (
    <Router root={App}>
      <Route path="/" component={Catalog} />
    </Router>
  ),
  root,
);
