import { defineConfig, devices } from "@playwright/test";

// End-to-end tests against the real API serving the built UI (`npm run e2e`
// builds it first). The large queue size keeps searches running until their
// time limit, so the live view and Stop can be tested.
const port = 8092;

export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
  },
  projects: [
    { name: "desktop", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } } },
    { name: "tablet", use: { ...devices["Desktop Chrome"], viewport: { width: 820, height: 1180 }, hasTouch: true } },
  ],
  webServer: {
    command: `cargo run --release -q -p api -- --listen 127.0.0.1:${port} --web-dir web/dist --queue-size 1000000`,
    cwd: "..",
    url: `http://127.0.0.1:${port}/openapi.json`,
    timeout: 600_000,
    reuseExistingServer: false,
  },
});
