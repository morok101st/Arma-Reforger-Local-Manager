import { defineConfig } from "vite";

// Die Oberfläche liegt in ui/, Tauri lädt im Paket den Inhalt von ui/dist.
export default defineConfig({
  root: "ui",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
  },
});
