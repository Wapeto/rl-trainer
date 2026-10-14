import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

// The phone page: `phone/index.html` builds to `dist/phone/`, which rl-app embeds.
export default defineConfig({
  root: here("./phone"),
  base: "/",
  publicDir: false,
  plugins: [react()],
  resolve: {
    alias: { "@shared": here("./src/shared"), "@phone": here("./src/phone") },
  },
  build: {
    outDir: here("./dist/phone"),
    emptyOutDir: true,
    // Fonts stay files: the CSP allows 'self' but not data: fonts.
    assetsInlineLimit: 0,
  },
  server: {
    port: 5173,
    proxy: { "/api": { target: "http://127.0.0.1:49321", ws: true } },
  },
});
