import { defineConfig } from "vite";

export default defineConfig({
  root: "src",
  publicDir: false,
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "../dist",
    target: "es2022",
    emptyOutDir: true,
  },
});
