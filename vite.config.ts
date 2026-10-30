import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed port, and the frontend must not clear the screen
// while the Rust side is streaming logs.
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      $lib: fileURLToPath(new URL("./src/lib", import.meta.url)),
      $state: fileURLToPath(new URL("./src/state", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    // The webview and tooling dial 127.0.0.1 explicitly; bind IPv4 only.
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    // Vite 8 minifies with oxc by default; esbuild is no longer bundled.
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
