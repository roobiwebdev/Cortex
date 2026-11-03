import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The app version lives in src-tauri/tauri.conf.json (scripts/desktop/bump-version.sh keeps
// package.json and Cargo.toml in step). The UI reads it as import.meta.env.CORTEX_APP_VERSION.
function appVersion(): string {
  const conf: unknown = JSON.parse(readFileSync(new URL("./src-tauri/tauri.conf.json", import.meta.url), "utf8"));
  const version = typeof conf === "object" && conf !== null ? Reflect.get(conf, "version") : undefined;
  if (typeof version !== "string") throw new Error("tauri.conf.json has no version");
  return version;
}

// The desktop UI runs in a plain browser during design review (mock engine),
// and later inside the Tauri webview with the real engine behind IPC.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { strictPort: true },
  define: { "import.meta.env.CORTEX_APP_VERSION": JSON.stringify(appVersion()) },
});
