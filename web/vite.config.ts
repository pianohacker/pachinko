import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { patchCssModules } from "vite-css-modules";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    patchCssModules({
      generateSourceTypes: true,
    }),
  ],
  css: {
    modules: {},
  },
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:7224",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
});
