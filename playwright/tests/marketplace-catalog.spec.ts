import { test, expect } from '@playwright/test';

// The catalogue pages are auth-gated SSR. Unauthenticated access must redirect
// to login rather than leak catalogue content; the login page itself must not
// contain plugin markup.

test('catalog plugins page is auth-gated', async ({ page }) => {
  await page.goto('/admin/catalog/plugins');
  await expect(page).toHaveURL(/login/);
  await expect(page.getByText('astound-dev')).toHaveCount(0);
});

test('catalog skills page is auth-gated', async ({ page }) => {
  await page.goto('/admin/catalog/skills');
  await expect(page).toHaveURL(/login/);
});

test('bridge plugin files require a token', async ({ request }) => {
  const response = await request.get(
    '/v1/bridge/plugins/astound-dev/.claude-plugin/plugin.json'
  );
  expect([401, 403]).toContain(response.status());
});
