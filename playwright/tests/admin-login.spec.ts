import { test, expect } from '@playwright/test';

// The admin login page is the one public admin surface. It must render the
// sign-in options without auth; everything behind it must bounce back here.

test('login page renders with sign-in options', async ({ page }) => {
  const response = await page.goto('/admin/login');
  expect(response?.status()).toBe(200);
  await expect(page).toHaveTitle(/sign in|log in|systemprompt/i);
  await expect(page.locator('form, [data-testid], main').first()).toBeVisible();
});

test('admin dashboard is auth-gated', async ({ page }) => {
  await page.goto('/admin');
  await expect(page).toHaveURL(/login/);
});
