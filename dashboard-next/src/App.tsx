import type { Component } from "solid-js";
import { HashRouter } from "@solidjs/router";
import { routes } from "./routes";
import { ToastHost } from "@/components/Toast";

const App: Component = () => (
  <>
    <HashRouter>{routes}</HashRouter>
    <ToastHost />
  </>
);

export default App;
