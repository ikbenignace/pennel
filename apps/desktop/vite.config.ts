import { resolve } from "node:path";
import { defineConfig } from "vite";

const root = import.meta.dirname;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: [resolve(root, "../..")] },
  },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        main: resolve(root, "index.html"),
        pill: resolve(root, "pill.html"),
      },
    },
  },
});
