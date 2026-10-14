import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

// The desktop window: `desktop/index.html` builds to `dist/desktop/`, which Tauri loads and
// `rl-replay serve --desktop-port` serves. `npm run dev:desktop` proxies the commands to that
// server (start it with `--desktop-port 49322`).
export default defineConfig({
  root: here("./desktop"),
  base: "./",
  publicDir: false,
  plugins: [react()],
  resolve: {
    alias: { "@shared": here("./src/shared"), "@desktop": here("./src/desktop") },
  },
  build: {
    outDir: here("./dist/desktop"),
    emptyOutDir: true,
    assetsInlineLimit: 0,
  },
  server: {
    port: 5174,
    proxy: { "/desktop-api": "http://127.0.0.1:49322" },
  },
});
