import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Don't watch the Rust build output; linking renamer.exe makes chokidar
    // crash with EBUSY while the exe is being written.
    watch: {
      ignored: ["**/src-tauri/target/**"],
    },
  },
  build: {
    target: "chrome105",
    minify: true,
  },
});
