import { test, expect } from '@playwright/test';

// The device-link page is the entry point of the developer onboarding flow
// (clean client / dev sandbox paste a code shown here). It must render up to
// the auth boundary.

test('device-link page renders', async ({ page }) => {
  const response = await page.goto('/bridge-auth/device-link');
  expect(response?.status()).toBeLessThan(500);
  await expect(page.locator('body')).toBeVisible();
});
