import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import eslint from "vite-plugin-eslint";

export default defineConfig({
  plugins: [react(), eslint()],
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:21337",
        changeOrigin: true,
      },
      "/api/v2/ws": {
        target: "ws://localhost:21337",
        ws: true,
      },
    },
    open: true,
  },
});
