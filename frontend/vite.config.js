import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import eslint from "vite-plugin-eslint";

export default defineConfig({
  plugins: [react(), eslint()],
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:21337", // Replace with your backend server URL
        changeOrigin: true,
      },
      "/api/ws": {
        target: "ws://localhost:21337", // Replace with your WebSocket server URL
        ws: true,
      },
    },
    open: true,
  },
});
