import { defineConfig, devices } from '@playwright/test';

// Runs against an already-running gateway (`just start`); it never boots one.
// GATEWAY_URL overrides the default local port.
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: process.env.GATEWAY_URL ?? 'http://localhost:8080',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
