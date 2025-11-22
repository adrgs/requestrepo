const { defineConfig } = require("vite");

module.exports = async () => {
  // Dynamically import the ESM-only plugin so this config works when Vite's
  // CJS Node API is used (some environments call require() on the config).
  const react = (await import("@vitejs/plugin-react")).default;
  const eslint = (await import("vite-plugin-eslint")).default || (await import("vite-plugin-eslint"));

  return defineConfig({
    plugins: [react(), eslint()],
    server: {
      proxy: {
        "/api": {
          target: "http://localhost:21337",
          changeOrigin: true,
        },
        "/api/ws2": {
          target: "ws://localhost:21337",
          ws: true,
        },
      },
      open: true,
    },
  });
};
