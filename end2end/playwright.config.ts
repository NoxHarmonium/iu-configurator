import { devices, defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  // Dev-mode SSR + WASM hydration can be slow; give each test plenty of room.
  timeout: 60 * 1000,
  expect: {
    timeout: 10000,
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: "html",
  use: {
    baseURL: process.env.BASE_URL || "http://localhost:3000",
    trace: "on-first-retry",
  },

  // On CI only run Chromium; locally run all three browsers.
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
