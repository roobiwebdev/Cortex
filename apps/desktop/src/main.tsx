import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import "@fontsource-variable/inter";
import "@fontsource-variable/bricolage-grotesque";
import "@fontsource-variable/jetbrains-mono";
import "./styles.css";

import { TauriWindow } from "./app/tauri-window";
import { connectEngine } from "./lib/engine";
import { loadPlatform } from "./lib/platform";
import { trackVisibility } from "./lib/visibility";
import { PrototypeShell } from "./prototype/shell";

async function mount(root: HTMLElement) {
  trackVisibility();
  // Inside Tauri each window renders only its own surface; a browser gets the review harness.
  if (isTauri()) {
    await Promise.all([connectEngine(), loadPlatform()]);
    const { label } = getCurrentWindow();
    createRoot(root).render(
      <StrictMode>
        <TauriWindow label={label} />
      </StrictMode>,
    );
    return;
  }
  createRoot(root).render(
    <StrictMode>
      <PrototypeShell />
    </StrictMode>,
  );
}

const root = document.getElementById("root");
if (root) void mount(root);
