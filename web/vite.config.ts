import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { patchCssModules } from "vite-css-modules";
import checker from "vite-plugin-checker";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    checker({
      typescript:
        process.env.NODE_ENV === "development"
          ? {
              buildMode: true, // Ensure it runs during build in development
            }
          : false,
      eslint: false,
    }),
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
