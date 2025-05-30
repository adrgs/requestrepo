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
        target: "ws://localhost:21337",
        ws: true,
        changeOrigin: true,
        secure: false,
        onError: (err) => {
          console.error(
            "WebSocket proxy error:",
            err?.message || "Unknown error",
          );
        },
        headers: {
          Origin: "http://localhost:21337",
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Credentials": "true",
        },
      },
    },
    open: true,
  },
  resolve: {
    extensions: [".mjs", ".js", ".ts", ".jsx", ".tsx", ".json"],
  },
});
