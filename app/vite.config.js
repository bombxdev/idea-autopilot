import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2021",
    rollupOptions: {
      input: { main: resolve(__dirname, "index.html"), capture: resolve(__dirname, "capture.html") },
    },
  },
});
