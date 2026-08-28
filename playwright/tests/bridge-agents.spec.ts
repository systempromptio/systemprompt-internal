import { test, expect, type Page } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

// Renders the bridge desktop GUI's Agents tab in a real browser, one shot per
// fixture, and asserts the things only a layout engine can answer.
//
// The bridge webview is Windows/macOS only and serves its assets over a wry
// custom protocol, so this drives `systemprompt-internal-bridge dev-web`
// instead — the same web tree over HTTP, with a mocked IPC fed by the fixtures
// in bin/bridge/web/dev/fixtures. See bin/bridge/README.md § Developing the GUI.
//
// Local and opt-in (`just bridge-shots`). CI has no browser, and cross-machine
// font rendering makes image diffing flaky, so nothing here compares pixels —
// the screenshots are for a human to look at; the assertions are structural.

const PREVIEW = process.env.BRIDGE_PREVIEW_URL ?? 'http://127.0.0.1:4310';
const SHOTS = path.resolve(__dirname, '../bridge-shots');

// Skip rather than fail when the preview is not up: this file shares a testDir
// with the gateway specs, and a bare `npx playwright test` has no reason to
// have started it. `just bridge-shots` always does.
let fixtures: string[] = [];
test.beforeAll(async ({ request }) => {
  try {
    const res = await request.get(`${PREVIEW}/dev/fixtures`, { timeout: 2000 });
    if (res.ok()) { fixtures = await res.json(); }
  } catch {
    fixtures = [];
  }
});

test.beforeAll(() => { fs.mkdirSync(SHOTS, { recursive: true }); });

/** Opens the Agents tab with the given fixture, failing on any page error. */
async function openAgents(page: Page, fixture: string): Promise<string[]> {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
  await page.addInitScript(() => localStorage.setItem('bridge.tab', 'agents'));
  await page.goto(`${PREVIEW}/?fixture=${fixture}`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  return errors;
}

test.describe('bridge Agents tab', () => {
  test('every fixture renders cleanly and is captured', async ({ page }) => {
    test.skip(fixtures.length === 0, `no preview at ${PREVIEW} — run \`just bridge-shots\``);
    await page.setViewportSize({ width: 1280, height: 860 });
    for (const fixture of fixtures) {
      const errors = await openAgents(page, fixture);
      await page.screenshot({ path: path.join(SHOTS, `${fixture}.png`) });
      expect(errors, `page errors on fixture "${fixture}"`).toEqual([]);
      // Wide values — registry paths, UUIDs, model lists — are what used to
      // push this page sideways.
      const overflow = await page.evaluate(() =>
        document.documentElement.scrollWidth > document.documentElement.clientWidth);
      expect(overflow, `horizontal overflow on fixture "${fixture}"`).toBe(false);
    }
  });

  test('an agent with no profile is offered, not listed', async ({ page }) => {
    test.skip(!fixtures.includes('app-missing'), 'app-missing fixture absent');
    await openAgents(page, 'app-missing');
    // Codex CLI is not set up on this machine: it has no status worth
    // reporting, so it belongs behind "Add agent" rather than in the list.
    await expect(page.locator('sp-agent-row')).toHaveCount(1);
    await expect(page.locator('sp-agent-row')).toContainText('Claude Desktop');
    await page.locator('[data-action="add-agent"]').first().click();
    const picker = page.locator('[role="dialog"]');
    await expect(picker).toBeVisible();
    await expect(picker).toContainText('Codex CLI');
    await expect(picker).toContainText('Download');
    await page.screenshot({ path: path.join(SHOTS, 'app-missing--add.png') });
  });

  test('before the first sync the picker says the list is provisional', async ({ page }) => {
    test.skip(!fixtures.includes('not-synced'), 'not-synced fixture absent');
    await openAgents(page, 'not-synced');
    await page.locator('[data-action="add-agent"]').first().click();
    const picker = page.locator('[role="dialog"]');
    await expect(picker).toBeVisible();
    // hosts_gated=false means the list is every host this build registers, not
    // what the installation permits. Saying so beats implying authority.
    await expect(picker).toContainText('provisional');
    await page.screenshot({ path: path.join(SHOTS, 'not-synced--add.png') });
  });

  test('the drawer holds the technical detail and honours the keyboard', async ({ page }) => {
    test.skip(!fixtures.includes('healthy'), 'healthy fixture absent');
    await openAgents(page, 'healthy');
    await page.locator('.sp-agent-row__main').first().click();
    const drawer = page.locator('[role="dialog"]');
    await expect(drawer).toBeVisible();
    await page.screenshot({ path: path.join(SHOTS, 'healthy--drawer.png') });

    // Everything the old card showed on the first screen still exists, one
    // click away, rather than having been dropped in the redesign.
    const advanced = drawer.locator('summary', { hasText: 'Technical detail' });
    await advanced.click();
    for (const value of ['Config location', 'Profile UUID', 'Payload UUID', 'Host kind',
                         'Config format', 'Resolved profile keys']) {
      await expect(drawer).toContainText(value);
    }
    await expect(drawer).toContainText('Model filter');
    // The section is below the fold of the drawer's own scroll area. Without
    // this the screenshot shows the top of the drawer and silently fails to
    // evidence the thing it is named for.
    await page.locator('.sp-drawer__body').evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(SHOTS, 'healthy--drawer-advanced.png') });

    await page.keyboard.press('Escape');
    await expect(page.locator('[role="dialog"]')).toHaveCount(0);
    // Focus must land back on the row that opened it, not at the top of the page.
    const focused = await page.evaluate(() => document.activeElement?.className ?? '');
    expect(focused).toContain('sp-agent-row__main');
  });

  test('repair is one press, not generate-then-install', async ({ page }) => {
    test.skip(!fixtures.includes('stale'), 'stale fixture absent');
    await openAgents(page, 'stale');
    const row = page.locator('sp-agent-row').first();
    await expect(row).toContainText('Needs attention');
    await row.locator('[data-kind="repair"]').click();
    await expect(row).toContainText('Working', { timeout: 5000 });
    await page.screenshot({ path: path.join(SHOTS, 'stale--after-repair.png') });
  });
});
