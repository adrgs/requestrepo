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
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    monacoEditorPlugin({
      languageWorkers: ["editorWorkerService", "json", "html", "css"],
    }),
  ],
  envDir: path.resolve(import.meta.dirname, ".."),
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
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
