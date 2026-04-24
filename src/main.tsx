import React from "react";
import ReactDOM from "react-dom/client";
import { restoreStateCurrent, StateFlags } from "@tauri-apps/plugin-window-state";
import App from "./App";

restoreStateCurrent(StateFlags.ALL);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
