import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  cacheDir: join(
    tmpdir(),
    `klickwerk-vite-${createHash("sha256").update(process.cwd()).digest("hex").slice(0, 8)}`,
  ),
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      usePolling: process.env.VITE_USE_POLLING === "true",
      interval: 500,
      ignored: ["**/src-tauri/target/**", "**/build/**", "**/test-results/**"],
    },
  },
  build: { target: "es2022", sourcemap: false },
});
