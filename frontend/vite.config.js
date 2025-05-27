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
      "/api/ws2": {
        target: "ws://localhost:21337", // Replace with your WebSocket server URL
        ws: true,
        changeOrigin: true,
        secure: false,
        rewrite: (path) => path.replace(/^\/api\/ws2/, "/api/ws2"),
        onError: () => {
          console.error("WebSocket proxy error (limited logging)");
        },
      },
    },
    open: true,
  },
  resolve: {
    extensions: [".mjs", ".js", ".ts", ".jsx", ".tsx", ".json"],
  },
});
