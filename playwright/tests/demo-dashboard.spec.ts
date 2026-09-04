import { test, expect, type Page } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

// Logged-in screenshots of the Demo dashboards as both roles, plus the
// assertions only a real browser session can make: that the two `me` pages
// show different people with different numbers, that a non-admin is redirected
// off the admin pages and never sees the nav entry, and that the governed
// calls the seed made are visible on the page.
//
// Prerequisites: a running stack (`just start`) seeded by
// `just e2e-live-demo-seed`. Run with `just demo-shots`.

const SHOTS = path.resolve(__dirname, '../demo-shots');
const PASSWORD = process.env.DEMO_PASSWORD ?? 'e2e-live-password-2026';
const ADMIN_LOGIN = process.env.DEMO_ADMIN_LOGIN ?? 'ed@systemprompt.io';
const USER_LOGIN = process.env.DEMO_USER_LOGIN ?? 'ed+notadmin@systemprompt.io';

const NARROW = { width: 375, height: 1200 };
const DESKTOP = { width: 1440, height: 1000 };

async function login(page: Page, login: string) {
  await page.goto('/admin/login');
  await page.fill('#odoo-login', login);
  await page.fill('#odoo-credential', PASSWORD);
  await page.click('#odoo-sign-in-btn');
  await page.waitForURL((url) => !url.pathname.endsWith('/admin/login'), { timeout: 20_000 });
}

// Both viewports of one page, into demo-shots/<role>/<name>-{dark,narrow}.png.
// The admin theme is dark-only, so a light capture would be byte-identical.
async function shoot(page: Page, role: string, name: string) {
  const dir = path.join(SHOTS, role);
  fs.mkdirSync(dir, { recursive: true });
  await page.setViewportSize(DESKTOP);
  await page.screenshot({ path: path.join(dir, `${name}-dark.png`), fullPage: true });
  await page.setViewportSize(NARROW);
  await page.screenshot({ path: path.join(dir, `${name}-narrow.png`), fullPage: true });
  await page.setViewportSize(DESKTOP);
}

async function kpi(page: Page, testId: string): Promise<number> {
  const text = await page.getByTestId(testId).innerText();
  const digits = text.replace(/[^0-9]/g, '');
  return digits === '' ? 0 : Number(digits);
}

test.describe('demo dashboards', () => {
  test.describe.configure({ mode: 'serial' });

  let adminSkillInvocations = -1;
  let userSkillInvocations = -1;

  test('admin sees all four pages', async ({ page }) => {
    await login(page, ADMIN_LOGIN);

    for (const [route, name] of [
      ['/admin/demo', 'logbook'],
      ['/admin/demo/skills', 'skills'],
      ['/admin/demo/tools', 'tools'],
      ['/admin/demo/me', 'me'],
    ] as const) {
      const response = await page.goto(route);
      expect(response?.status(), `${route} is 200 for the admin`).toBe(200);
      expect(new URL(page.url()).pathname, `${route} does not redirect the admin`).toBe(route);
      await shoot(page, 'admin', name);
    }

    await page.goto('/admin/demo/me');
    await expect(page.getByTestId('demo-me-email')).toHaveText(ADMIN_LOGIN);
    adminSkillInvocations = await kpi(page, 'demo-kpi-skill-invocations');
    expect(adminSkillInvocations, 'the admin ran skills in the seed').toBeGreaterThan(0);
    expect(await kpi(page, 'demo-kpi-mcp-calls'), 'the admin made MCP calls').toBeGreaterThan(0);

    await page.goto('/admin/demo');
    await expect(page.locator('body')).toContainText('tool_blocklist');
  });

  test('non-admin sees only their own usage', async ({ page }) => {
    await login(page, USER_LOGIN);

    const response = await page.goto('/admin/demo/me');
    expect(response?.status(), '/admin/demo/me is 200 for a non-admin').toBe(200);
    expect(new URL(page.url()).pathname).toBe('/admin/demo/me');
    await expect(page.getByTestId('demo-me-email')).toHaveText(USER_LOGIN);

    userSkillInvocations = await kpi(page, 'demo-kpi-skill-invocations');
    expect(userSkillInvocations, 'the salesperson ran skills in the seed').toBeGreaterThan(0);
    expect(await kpi(page, 'demo-kpi-mcp-calls')).toBeGreaterThan(0);

    // The seed gives the two roles deliberately different work; equal counts
    // mean the page is not scoped to the signed-in user.
    expect(userSkillInvocations, 'the two roles show different totals').not.toBe(adminSkillInvocations);

    const body = page.locator('body');
    await expect(body, 'the blocked delete shows as denied').toContainText(/deny/i);
    await expect(body, 'the held write shows as pending').toContainText(/pending/i);
    await expect(page.locator('nav'), 'no admin Demo links in the sidebar').not.toContainText('Logbook');

    await shoot(page, 'user', 'me');

    for (const route of ['/admin/demo', '/admin/demo/tools']) {
      await page.goto(route);
      expect(new URL(page.url()).pathname, `${route} redirects a non-admin`).toBe('/admin/profile');
    }
  });
});
