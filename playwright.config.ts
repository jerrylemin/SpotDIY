import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/playwright",
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:4173",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    video: "off",
  },
  webServer: {
    command: "pnpm exec vite --host 127.0.0.1 --port 4173",
    env: {
      ...process.env,
      VITE_SPOTDIY_E2E: "1",
    },
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    url: "http://127.0.0.1:4173",
  },
  projects: [
    {
      name: "visual-1280",
      testIgnore: /plan15-visual-exploration\.spec\.ts/,
      use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 720 } },
    },
    {
      name: "visual-1920",
      testIgnore: /plan15-visual-exploration\.spec\.ts/,
      use: { ...devices["Desktop Chrome"], viewport: { width: 1920, height: 1080 } },
    },
    {
      name: "visual-2560",
      testIgnore: /plan15-visual-exploration\.spec\.ts/,
      use: { ...devices["Desktop Chrome"], viewport: { width: 2560, height: 1440 } },
    },
    {
      name: "plan15-ultrawide",
      testMatch: /plan15-visual-exploration\.spec\.ts/,
      use: { ...devices["Desktop Chrome"], viewport: { width: 3440, height: 1440 } },
    },
  ],
});
