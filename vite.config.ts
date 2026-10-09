import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and fails if it is already taken.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // src-tauri is watched by cargo, not vite.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "chrome105",
    sourcemap: false,
  },
});
