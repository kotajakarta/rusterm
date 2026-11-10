import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Dev: run `python termdeck.py --no-browser --token dev`, then open
// http://localhost:5173/?token=dev
const backend = "http://127.0.0.1:9095";

export default defineConfig({
  plugins: [react()],
  base: "./",
  server: {
    proxy: {
      "/api": backend,
      "/ws": { target: backend, ws: true },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
