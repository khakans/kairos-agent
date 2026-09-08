import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";
const port = 18080;
export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  timeout: 45000,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    channel: existsSync("C:/Program Files/Google/Chrome/Application/chrome.exe")
      ? "chrome"
      : undefined,
    viewport: { width: 1440, height: 1000 },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  reporter: [["list"], ["html", { open: "never" }]],
  webServer: [
    {
      command: "node tests/upstream.mjs",
      url: "http://127.0.0.1:18081/health",
      reuseExistingServer: false,
    },
    {
      command: "cargo run -p kairos-server --features test-support",
      url: `http://127.0.0.1:${port}/health/live`,
      reuseExistingServer: false,
      timeout: 120000,
      env: {
        KAIROS_BIND: `127.0.0.1:${port}`,
        KAIROS_DATA_DIR: `.smoke-data-${Date.now()}`,
        KAIROS_OWNER_TOKEN: "smoke-only-owner-token-not-for-production",
        KAIROS_TEST_UPSTREAM: "http://127.0.0.1:18081",
      },
    },
  ],
});
