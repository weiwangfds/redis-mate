import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";
import "./i18n";

async function initLogging() {
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const hasTauri = typeof window !== "undefined" && (window as any).__TAURI__;
    if (hasTauri) {
      const { attachConsole } = await import("@tauri-apps/plugin-log");
      await attachConsole();
    }
  } catch {
    // ignore when not running inside Tauri
  }
}

void initLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
