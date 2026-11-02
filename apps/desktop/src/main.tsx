import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@fontsource-variable/inter";
import "@fontsource-variable/bricolage-grotesque";
import "@fontsource-variable/jetbrains-mono";
import "./styles.css";

import { PrototypeShell } from "./prototype/shell";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <PrototypeShell />
    </StrictMode>,
  );
}
