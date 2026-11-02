import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The desktop UI runs in a plain browser during design review (mock engine),
// and later inside the Tauri webview with the real engine behind IPC.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { strictPort: true },
});
