import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// The API the dev server forwards to (`cargo run -p api`). In production
// the API serves the built UI itself, on the same origin (docs/API.md,
// "Serving the UI"), so the UI always calls relative URLs.
const api = process.env.SHEETSOLVER_API ?? "http://127.0.0.1:8080";

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      "/jobs": { target: api, changeOrigin: true },
      "/openapi.json": { target: api, changeOrigin: true },
    },
  },
  build: { target: "es2022" },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
