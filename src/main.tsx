import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { isLinux } from "#lib/platform.ts";

// WebKitGTK (Linux Tauri webview) mis-composites `backdrop-filter`, ghosting
// text painted over it. Tag the root so CSS can drop blur on Linux only.
if (isLinux) {
  document.documentElement.classList.add("platform-linux");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
