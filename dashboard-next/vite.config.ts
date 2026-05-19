import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import checker from "vite-plugin-checker";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  plugins: [
    solid(),
    checker({
      typescript: true,
    }),
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
  },
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:6185",
        changeOrigin: true,
      },
      "/webchat": {
        target: "http://127.0.0.1:6185",
        changeOrigin: true,
      },
      "/ws": {
        target: "ws://127.0.0.1:6185",
        ws: true,
        changeOrigin: true,
      },
    },
  },
});
