import { test, expect, type Page } from '@playwright/test';

// Evidence for bridge-review doc 01 (navigation and information architecture).
//
// The findings this file pins down are the ones a compiler cannot see: that
// Home is short when nothing is wrong and grows when something is, that the
// rail never truncates its own labels, that a shortcut advertised on seven
// panes works on seven panes, and that the three-way scheduler status renders
// as three distinct things rather than two.
//
// Local and opt-in (`just bridge-nav-shots`). Nothing compares pixels.

const PREVIEW = process.env.BRIDGE_PREVIEW_URL ?? 'http://127.0.0.1:4310';

const TABS = ['home', 'agents', 'activity', 'marketplace', 'account', 'settings', 'status'];

let up = false;
test.beforeAll(async ({ request }) => {
  try {
    up = (await request.get(`${PREVIEW}/dev/fixtures`, { timeout: 2000 })).ok();
  } catch {
    up = false;
  }
});

async function open(page: Page, fixture: string) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(`${PREVIEW}/?fixture=${fixture}`, { waitUntil: 'networkidle' });
  // Some fixtures legitimately render the first-run overlay instead of the
  // shell, and the shell is `display: none` behind it -- so wait for whichever
  // one this fixture is showing rather than assuming the rail.
  await page.waitForSelector('.sp-rail-tab, sp-setup:not([hidden])', { state: 'attached' });
  await page.waitForFunction(() =>
    document.body.classList.contains('is-setup-mode')
    || !!document.querySelector('.sp-rail-tab'));
  return errors;
}

async function inSetupMode(page: Page) {
  return page.evaluate(() => document.body.classList.contains('is-setup-mode'));
}

test.describe('bridge navigation and IA', () => {
  test.beforeEach(({}, testInfo) => {
    test.skip(!up, `preview not running at ${PREVIEW}`);
    testInfo.setTimeout(120_000);
  });

  test('opens on Home, not on a reporting pane', async ({ page }) => {
    await open(page, 'healthy');
    await expect(page.locator('.sp-rail-tab[aria-selected="true"]')).toHaveAttribute('data-tab', 'home');
    await expect(page.locator('#sp-panel-home')).toBeVisible();
  });

  test('the rail never truncates its own labels', async ({ page }) => {
    await open(page, 'healthy');
    const truncated = () => page.$$eval('.sp-rail-tab__label',
      (ns) => ns.filter((n) => n.scrollWidth > n.clientWidth + 1).map((n) => n.textContent));
    expect(await truncated(), 'primary navigation must not ellipsise').toEqual([]);

    const glyphs = await page.$$eval('.sp-rail-tab__glyph', (ns) => ns.map((n) => n.textContent?.trim()));
    expect(glyphs, 'every tab needs a glyph entry').not.toContain('undefined');

    // The rail is content-sized, so this has to hold for a label longer than
    // any English one -- a fixed width is only ever correct for the language it
    // was measured in, and the next catalogue is the one that breaks it.
    const before = await page.$eval('sp-rail', (n) => n.getBoundingClientRect().width);
    await page.$eval('.sp-rail-tab[data-tab="marketplace"] .sp-rail-tab__label',
      (n) => { n.textContent = 'Marktplatz für Erweiterungen'; });
    await expect.poll(async () => (await truncated()).length,
      { message: 'a longer label must widen the rail, not ellipsise' }).toBe(0);
    expect(await page.$eval('sp-rail', (n) => n.getBoundingClientRect().width)).toBeGreaterThan(before);
  });

  test('every tab is wired to its panel for assistive tech', async ({ page }) => {
    await open(page, 'healthy');
    for (const tab of TABS) {
      await expect(page.locator(`.sp-rail-tab[data-tab="${tab}"]`)).toHaveAttribute('aria-controls', `sp-panel-${tab}`);
      await expect(page.locator(`#sp-panel-${tab}`)).toHaveAttribute('aria-labelledby', `sp-tab-${tab}`);
    }
    await expect(page.locator('sp-rail')).toHaveAttribute('aria-orientation', 'vertical');
  });

  test('activating a tab moves focus into the panel', async ({ page }) => {
    await open(page, 'healthy');
    await page.click('.sp-rail-tab[data-tab="settings"]');
    await expect.poll(() => page.evaluate(() => document.activeElement?.id)).toBe('sp-panel-settings');
  });

  // The review's rule for the pane: when everything is fine it is short and
  // boring, and it grows only when something needs a person.
  test('Home is quiet when healthy and names the problem when not', async ({ page }) => {
    await open(page, 'healthy');
    await expect(page.locator('#sp-panel-home .sp-home__waiting-item')).toHaveCount(0);
    await expect(page.locator('#sp-panel-home .sp-home__agent[data-state="ok"]')).toHaveCount(2);

    await open(page, 'stale');
    const rows = page.locator('#sp-panel-home .sp-home__agent');
    await expect(rows.first()).toHaveAttribute('data-state', 'attention');
    await expect(rows.first()).toContainText('out of date');

    await open(page, 'not-synced');
    await expect(page.locator('#sp-panel-home .sp-home__waiting-item')).toContainText('not synced');
  });

  test('the governance strip degrades honestly', async ({ page }) => {
    for (const [fixture, state] of [['governing', 'ok'], ['no-traffic', 'idle'], ['proxy-unreachable', 'down']]) {
      await open(page, fixture);
      await expect(page.locator('#sp-panel-home sp-governance-strip')).toHaveAttribute('data-state', state);
    }
    await expect(page.locator('#sp-panel-home sp-governance-strip'))
      .toContainText('are not being governed');
  });

  test('the search shortcut works from every pane, not just Marketplace', async ({ page }) => {
    await open(page, 'healthy');
    for (const from of ['home', 'agents', 'settings', 'status']) {
      await page.click(`.sp-rail-tab[data-tab="${from}"]`);
      await page.keyboard.press('Control+f');
      await expect(page.locator('.sp-rail-tab[aria-selected="true"]')).toHaveAttribute('data-tab', 'marketplace');
      await expect.poll(() => page.evaluate(() => document.activeElement?.id)).toBe('mkt-search');
    }
  });

  test('the Status cross-navigation link is not inert', async ({ page }) => {
    await open(page, 'healthy');
    await page.click('.sp-rail-tab[data-tab="status"]');
    await page.click('[data-jump-tab="agents"]');
    await expect(page.locator('.sp-rail-tab[aria-selected="true"]')).toHaveAttribute('data-tab', 'agents');
  });

  test('Settings refuses to save over a config it could not read', async ({ page }) => {
    await open(page, 'config-malformed');
    await page.click('.sp-rail-tab[data-tab="settings"]');
    await expect(page.locator('.sp-settings__banner')).toContainText('could not be read');
    await expect(page.locator('.sp-settings__banner')).toContainText('line 4');
  });

  test('a policy-set signing key says so', async ({ page }) => {
    await open(page, 'healthy');
    await page.click('.sp-rail-tab[data-tab="settings"]');
    await expect(page.locator('.sp-settings__section .sp-badge')).toContainText('device policy');
  });

  // Installed / not installed / unknown must render as three things. Collapsing
  // "the scheduler would not answer" into "not installed" is the guess the
  // hardcoded string used to make.
  test('an undeterminable sync schedule says so rather than guessing', async ({ page }) => {
    await open(page, 'healthy');
    await page.click('.sp-rail-tab[data-tab="settings"]');
    await expect(page.locator('#sp-panel-settings')).toContainText('Manual');

    await open(page, 'config-malformed');
    await page.click('.sp-rail-tab[data-tab="settings"]');
    await expect(page.locator('#sp-panel-settings')).toContainText('Could not be determined');
  });

  test('the gateway is editable in place, with validation', async ({ page }) => {
    await open(page, 'healthy');
    await page.click('.sp-rail-tab[data-tab="settings"]');
    await page.click('[data-action="edit-gateway"]');
    await page.fill('#settings-gateway', 'http://evil.example.com');
    await expect(page.locator('#settings-gateway-error')).toBeVisible();
    await expect(page.locator('[data-action="save-gateway"]')).toBeDisabled();
    await page.fill('#settings-gateway', 'https://gateway.example.com');
    await expect(page.locator('[data-action="save-gateway"]')).toBeEnabled();
    // The old control entered the first-run wizard, which has no way out.
    await expect(page.locator('body')).not.toHaveClass(/is-setup-mode/);
  });

  // Assert the accessible NAME, not the presence of the label element. A spec
  // that checks the label is hidden at this width passes whether the name
  // survives or not -- which is how seven unnamed tabs shipped.
  test('the icon-only rail keeps its accessible names', async ({ page }) => {
    await open(page, 'healthy');
    for (const width of [900, 820]) {
      await page.setViewportSize({ width, height: 850 });
      const names = await Promise.all(
        TABS.map((tab) => page.locator(`.sp-rail-tab[data-tab="${tab}"]`).evaluate(
          (n) => (n.getAttribute('aria-label') ?? n.textContent ?? '').trim())));
      for (const [i, name] of names.entries()) {
        expect(name, `tab "${TABS[i]}" has no accessible name at ${width}px`).not.toBe('');
      }
      // The account trigger loses its visible identity at this width too, and
      // its aria-label lives on the host element, not the button.
      const account = page.locator('.sp-rail-profile__trigger');
      await expect(account, `account trigger unnamed at ${width}px`)
        .toHaveAttribute('aria-label', /\S/);
    }
  });

  test('seven tabs still fit when the rail collapses to icons', async ({ page }) => {
    await open(page, 'healthy');
    await page.setViewportSize({ width: 900, height: 800 });
    await expect(page.locator('.sp-rail-tab')).toHaveCount(7);
    const railWidth = await page.$eval('sp-rail', (n) => Math.round(n.getBoundingClientRect().width));
    expect(railWidth).toBe(64);
    expect(await page.$eval('sp-rail', (n) => n.scrollHeight > n.clientHeight + 1)).toBe(false);
  });

  // The general form of the two bugs above: a name attached to the custom
  // element rather than to the thing carrying the role, where it is ignored.
  // One query over the whole tree catches those and whatever either document
  // adds next -- roled regions live inside hidden panels until their tab is
  // selected, so this has to visit every pane rather than only the default one.
  test('every roled element has an accessible name', async ({ page }) => {
    const unnamed: string[] = [];
    let inspected = 0;
    for (const width of [1400, 900]) {
      await open(page, 'healthy');
      await page.setViewportSize({ width, height: 850 });
      for (const tab of TABS) {
        await page.click(`.sp-rail-tab[data-tab="${tab}"]`);
        await page.waitForTimeout(250);
        const result = await page.evaluate(({ tab, width }) => {
          // Roles whose name comes from elsewhere, and are not defects when
          // unnamed: structural children named by their container, and the
          // ephemeral live regions (`status`, `alert`) that are announced by
          // their content the moment it changes rather than identified out of
          // context. `log` is deliberately NOT here -- it persists and is
          // navigable, so it needs a name of its own.
          const SKIP = new Set(['presentation', 'none', 'tabpanel', 'option', 'row',
            'cell', 'columnheader', 'rowgroup', 'list', 'listitem', 'paragraph',
            'generic', 'status', 'alert']);
          const out: string[] = [];
          let seen = 0;
          for (const el of Array.from(document.querySelectorAll('[role]'))) {
            const role = el.getAttribute('role') ?? '';
            if (SKIP.has(role)) { continue; }
            const html = el as HTMLElement;
            if (!html.offsetParent && getComputedStyle(el).position !== 'fixed') { continue; }
            seen += 1;
            const labelledby = el.getAttribute('aria-labelledby');
            const name = (el.getAttribute('aria-label')
              ?? (labelledby ? document.getElementById(labelledby)?.textContent : null)
              ?? html.innerText ?? '').trim();
            if (!name) {
              out.push(`${width}px ${tab}: role="${role}" <${el.tagName.toLowerCase()} class="${el.className}">`);
            }
          }
          return { out, seen };
        }, { tab, width });
        unnamed.push(...result.out);
        inspected += result.seen;
      }
    }
    expect(unnamed, 'a role with no accessible name is invisible to assistive tech').toEqual([]);
    // Guard against the check quietly inspecting nothing and passing.
    expect(inspected, 'the roled-element sweep found nothing to check').toBeGreaterThan(20);
  });

  test('no fixture renders with a page error', async ({ page, request }) => {
    const fixtures: string[] = await (await request.get(`${PREVIEW}/dev/fixtures`)).json();
    for (const fixture of fixtures) {
      const errors = await open(page, fixture);
      await page.waitForTimeout(300);
      expect(errors, `${fixture} rendered with errors`).toEqual([]);
      expect(await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth),
        `${fixture} overflows horizontally`).toBe(false);
      if (!await inSetupMode(page)) {
        expect(await page.$$eval('.sp-rail-tab__label',
          (ns) => ns.filter((n) => n.scrollWidth > n.clientWidth + 1).length),
          `${fixture} truncates a rail label`).toBe(0);
      }
    }
  });
});
