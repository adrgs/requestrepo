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
        rewrite: (path) => path.replace(/^\/api\/ws2/, "/ws2"),
        onError: (err) => {
          console.error("WebSocket proxy error:", err?.message || "Unknown error");
        },
        headers: {
          "Origin": "http://localhost:21337"
        },
        onProxyReqWs: (proxyReq) => {
          console.log("WebSocket proxy request:", proxyReq.path);
        }
      },
    },
    open: true,
  },
  resolve: {
    extensions: [".mjs", ".js", ".ts", ".jsx", ".tsx", ".json"],
  },
});
