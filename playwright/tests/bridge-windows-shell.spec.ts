import { test, expect, type Page } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

// Evidence for bridge-review doc 04 (the Windows-native shell).
//
// IMPORTANT SCOPE LIMIT. Most of doc 04 is native chrome — the immersive dark
// title bar, the tray icon and menu, WinRT toasts, the MessageBoxW alert, the
// Task Scheduler logon task. None of that is web, so none of it can appear
// here; this file only evidences the two pieces that live in the web tree, and
// the platform is simulated rather than real (see `asPlatform`).
//
// Local and opt-in (`just bridge-windows-shots`). Nothing compares pixels.

const PREVIEW = process.env.BRIDGE_PREVIEW_URL ?? 'http://127.0.0.1:4310';
const SHOTS = path.resolve(__dirname, '../bridge-shots');

let up = false;
test.beforeAll(async ({ request }) => {
  try {
    up = (await request.get(`${PREVIEW}/dev/fixtures`, { timeout: 2000 })).ok();
  } catch {
    up = false;
  }
});

test.beforeAll(() => { fs.mkdirSync(SHOTS, { recursive: true }); });

// The preview binary is built for Linux, so `render_index` stamps the body
// `is-platform-linux`. Rewriting the two attributes it substitutes is exactly
// what the Rust would emit on Windows, and it is the only way to see the
// platform-dependent copy from here.
async function asPlatform(page: Page, slug: string, display: string) {
  await page.route('**/*', async (route, request) => {
    if (request.resourceType() !== 'document') { return route.continue(); }
    const res = await route.fetch();
    const body = (await res.text())
      .replace(/is-platform-\w+/g, `is-platform-${slug}`)
      .replace(/data-platform-display="[^"]*"/g, `data-platform-display="${display}"`)
      .replace(/data-platform="[^"]*"/g, `data-platform="${slug}"`);
    await route.fulfill({ response: res, body });
  });
}

async function openTab(page: Page, tab: string, fixture: string): Promise<string[]> {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
  await page.addInitScript((t) => localStorage.setItem('bridge.tab', t), tab);
  await page.goto(`${PREVIEW}/?fixture=${fixture}`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  return errors;
}

test.describe('doc 04 — the parts of the Windows shell that live in the web tree', () => {
  test.beforeEach(async ({ page }) => {
    test.skip(!up, `no preview at ${PREVIEW} — run \`just bridge-windows-shots\``);
    await page.setViewportSize({ width: 1280, height: 860 });
    // The app's palette follows `prefers-color-scheme`; Chromium defaults to
    // light, which is not what this shell normally looks like.
    await page.emulateMedia({ colorScheme: 'dark' });
  });

  // §2. The muda menu bar is no longer attached to the HWND on Windows, so
  // Settings and the three Help commands have to be reachable from the web UI
  // or they are gone.
  test('the topbar overflow menu carries the commands the Win32 menu bar used to', async ({ page }) => {
    await asPlatform(page, 'windows', 'Windows');
    const errors = await openTab(page, 'agents', 'healthy');
    expect(errors, 'page errors').toEqual([]);

    const trigger = page.locator('sp-topbar-menu [data-action="toggle-menu"]');
    await expect(trigger).toBeVisible();
    await expect(trigger).toHaveAttribute('aria-expanded', 'false');
    await trigger.click();

    const menu = page.locator('sp-topbar-menu [role="menu"]');
    await expect(menu).toBeVisible();
    for (const label of ['Settings', 'Open log folder', 'Export diagnostic bundle', 'Open config folder']) {
      await expect(menu).toContainText(label);
    }
    await page.screenshot({ path: path.join(SHOTS, 'doc04-topbar-menu-open.png') });

    // Escape closes it: with no menu bar this is the only way to these commands
    // by keyboard, so it must behave like a menu.
    await page.keyboard.press('Escape');
    await expect(page.locator('sp-topbar-menu [role="menu"]')).toHaveCount(0);
  });

  test('the overflow menu reaches Settings', async ({ page }) => {
    await asPlatform(page, 'windows', 'Windows');
    await openTab(page, 'agents', 'healthy');
    await page.locator('sp-topbar-menu [data-action="toggle-menu"]').click();
    await page.locator('sp-topbar-menu [data-action="open-settings"]').click();
    await expect(page.locator('section[data-tab="settings"]')).toBeVisible();
    await expect(page.locator('.sp-rail-tab[data-tab="settings"]')).toHaveAttribute('aria-selected', 'true');
    await page.screenshot({ path: path.join(SHOTS, 'doc04-settings-via-menu.png') });
  });

  // §5. Autostart is the governance fix, not a convenience: until the app is
  // opened the proxy is down and agents run ungoverned.
  test('Settings offers start-at-login, named for the platform', async ({ page }) => {
    await asPlatform(page, 'windows', 'Windows');
    const errors = await openTab(page, 'settings', 'healthy');
    expect(errors, 'page errors').toEqual([]);

    const autostart = page.locator('[data-action="toggle-autostart"]');
    await expect(autostart).toBeVisible();
    await expect(autostart).not.toBeChecked();
    await expect(page.locator('.sp-settings__prefs')).toContainText('Start with Windows');
    await expect(page.locator('[data-action="toggle-auto-update"]')).toBeVisible();
    await page.screenshot({ path: path.join(SHOTS, 'doc04-settings-windows.png') });

    // The checkbox renders from the handler's reply, not from its own DOM
    // state, so a machine that refuses `schtasks` cannot leave the UI claiming
    // a registration that never happened.
    await autostart.check();
    await expect(page.locator('[data-action="toggle-autostart"]')).toBeChecked();
    await page.screenshot({ path: path.join(SHOTS, 'doc04-settings-autostart-on.png') });
  });

  // The title bar is set from `Window::theme()`, not pinned dark: the web tree
  // has a real light theme, so pinning it would reproduce §2's mismatch with
  // the colours swapped. The native bar cannot appear in a browser shot; this
  // is the light theme it has to match.
  test('the shell has a genuine light theme for the title bar to follow', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await asPlatform(page, 'windows', 'Windows');
    await openTab(page, 'settings', 'healthy');
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await page.screenshot({ path: path.join(SHOTS, 'doc04-settings-windows-light.png') });
  });

  // The scheduler can decline to answer — `schtasks` missing from PATH, a home
  // directory that will not stat. That is neither on nor off, and rendering it
  // as off gives the user a box that silently refuses to tick.
  test('an autostart state the scheduler will not confirm says so instead of reading as off', async ({ page }) => {
    await asPlatform(page, 'windows', 'Windows');
    await page.addInitScript(() => {
      window.localStorage.setItem('bridge.dev.autostart', 'unknown');
    });
    await openTab(page, 'settings', 'healthy');
    const box = page.locator('[data-action="toggle-autostart"]');
    await expect(box).toBeDisabled();
    await expect(box).not.toBeChecked();
    await expect(page.locator('.sp-settings__pref.is-unavailable'))
      .toContainText('could not be determined');
    await page.screenshot({ path: path.join(SHOTS, 'doc04-settings-autostart-unknown.png') });
  });

  test('the same row names macOS on macOS', async ({ page }) => {
    await asPlatform(page, 'macos', 'macOS');
    await openTab(page, 'settings', 'healthy');
    await expect(page.locator('.sp-settings__prefs')).toContainText('Start with macOS');
    await page.screenshot({ path: path.join(SHOTS, 'doc04-settings-macos.png') });
  });

  // §7. `update.automatic` was a supported config key with no UI at all.
  test('automatic updates are reachable from Settings at last', async ({ page }) => {
    await asPlatform(page, 'windows', 'Windows');
    await openTab(page, 'settings', 'healthy');
    const auto = page.locator('[data-action="toggle-auto-update"]');
    await expect(auto).not.toBeChecked();
    await auto.check();
    await expect(page.locator('[data-action="toggle-auto-update"]')).toBeChecked();
  });

  // §2. The topbar carried `-webkit-app-region: drag` with `no-drag` children —
  // an Electron frameless-window feature that a wry-hosted child WebView2 never
  // honoured. Shipping it made the window look like an abandoned frameless
  // attempt; it is deleted, and must stay deleted until someone actually
  // implements WM_NCHITTEST.
  test('no dead frameless-window CSS survives in the topbar', async ({ page }) => {
    await openTab(page, 'agents', 'healthy');
    const regions = await page.evaluate(() => {
      const bar = document.querySelector('.sp-topbar');
      if (!bar) { return ['no topbar']; }
      const hits: string[] = [];
      for (const el of [bar, ...Array.from(bar.children)]) {
        const v = getComputedStyle(el).getPropertyValue('-webkit-app-region').trim();
        if (v && v !== 'none' && v !== '') { hits.push(`${el.className}: ${v}`); }
      }
      return hits;
    });
    expect(regions).toEqual([]);
  });
});
