import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import monacoEditorPluginModule from "vite-plugin-monaco-editor";
/* eslint-disable @typescript-eslint/no-explicit-any -- CJS module interop varies by runtime */
const monacoEditorPlugin: any =
  typeof monacoEditorPluginModule === "function"
    ? monacoEditorPluginModule
    : (monacoEditorPluginModule as any).default;
/* eslint-enable @typescript-eslint/no-explicit-any */
import path from "path";

export default defineConfig({
  plugins: [
    react(),
    monacoEditorPlugin({
      // Only include the workers we need (editorWorkerService is required)
      languageWorkers: ["editorWorkerService", "json", "html", "css"],
    }),
  ],
  // Load .env from parent directory (shared with backend for local dev)
  // In Docker builds, parent has no .env - Vite handles this gracefully
  envDir: path.resolve(__dirname, ".."),
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:21337",
        changeOrigin: true,
        ws: true,
      },
    },
  },
});
