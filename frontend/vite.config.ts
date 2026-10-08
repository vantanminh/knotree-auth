import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/oauth": {
        target: "http://127.0.0.1:8080",
        bypass(req) {
          const path = req.url?.split("?")[0] ?? "";
          if (path === "/oauth/consent" || path === "/oauth/error") return "/index.html";
        },
      },
      "/.well-known": "http://127.0.0.1:8080",
      "/health": "http://127.0.0.1:8080",
      "/ready": "http://127.0.0.1:8080",
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test-setup.ts",
    include: ["src/**/*.{test,spec}.{ts,tsx}", "worker/**/*.{test,spec}.ts"],
    exclude: ["e2e/**", "node_modules/**", "dist/**"],
  },
});
