import { test, expect, type Page } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

// Rasterizes every artifact the Rust render test wrote, in three viewports, and
// asserts the things only a real layout engine can answer: that nothing
// overflows at 375px, that the brand accent survives into computed style in
// both colour schemes, and that nothing renders as a zero-height blank.
//
// Local and opt-in (`just artifact-gallery`). CI has no browser, and
// cross-machine font rendering makes image diffing flaky — the functional
// assertions live in the Rust test, which does run in CI.

const GALLERY = process.env.ARTIFACT_GALLERY_DIR
  ?? path.resolve(__dirname, '../../target/artifact-gallery');
const SHOTS = path.resolve(__dirname, '../artifact-shots');

type Entry = { type: string; title: string; file: string };

// Skip rather than fail when the manifest is absent: this file shares a testDir
// with the gateway specs, and a bare `just playwright` has no reason to have
// run the Rust renderer. `just artifact-gallery` always runs it first.
const manifestPath = path.join(GALLERY, 'manifest.json');
const entries: Entry[] = fs.existsSync(manifestPath)
  ? JSON.parse(fs.readFileSync(manifestPath, 'utf8')).entries
  : [];
test.skip(entries.length === 0, `no ${manifestPath} — run \`just artifact-gallery\``);

// The wire test writes this one only when the Odoo MCP binary is present.
const wire = path.join(GALLERY, 'wire-crm-lead-search.html');
if (entries.length > 0 && fs.existsSync(wire)) {
  entries.push({ type: 'wire-crm-lead-search', title: 'crm_lead_search over the MCP wire', file: 'wire-crm-lead-search.html' });
}

if (entries.length > 0) fs.mkdirSync(SHOTS, { recursive: true });

// The brand orange, as `oklch(0.67 0.18 50)` comes back from getComputedStyle.
// Chromium normalises it to `oklch(L C H)` with its own precision, so match on
// the three numbers rather than the literal string, and allow the dark-scheme
// variant (0.72 0.17 52) too.
const ACCENT = /oklch\(0\.6[6-8]\d*\s+0\.1[78]\d*\s+5[01](\.\d+)?\)|oklch\(0\.7[12]\d*\s+0\.1[67]\d*\s+5[12](\.\d+)?\)/;

async function assertRenderedSanely(page: Page, label: string) {
  // A blank render: the body exists but everything inside it collapsed.
  const bodyHeight = await page.evaluate(() => document.body.getBoundingClientRect().height);
  expect(bodyHeight, `${label}: body has height`).toBeGreaterThan(20);

  const zeroHeight = await page.evaluate(() => {
    // Only HTML elements that actually generate a layout box are candidates.
    // SVG metadata (`<title>`, `<desc>`) and structural `<g>` nodes, `<option>`
    // inside a closed `<select>`, and the fallback content browsers never lay
    // out inside `<audio>`/`<video>` all legitimately measure zero — counting
    // them would make the check fire on healthy pages and mean nothing.
    const NO_BOX = new Set([
      'SCRIPT', 'STYLE', 'BR', 'TEMPLATE', 'META', 'LINK', 'SOURCE', 'TRACK',
      'HEAD', 'TITLE', 'OPTION', 'OPTGROUP', 'NOSCRIPT', 'PARAM',
    ]);
    const bad: string[] = [];
    for (const el of Array.from(document.querySelectorAll('body *'))) {
      if (el.namespaceURI !== 'http://www.w3.org/1999/xhtml') continue;
      if (NO_BOX.has(el.tagName)) continue;
      if (el.closest('audio, video, select, svg')) continue;
      const style = getComputedStyle(el);
      if (style.display === 'none' || style.visibility === 'hidden') continue;
      if (style.position === 'absolute' || style.position === 'fixed') continue;
      // Nothing to show: an empty wrapper collapsing is not a regression.
      if (el.textContent?.trim() === '' && el.children.length === 0) continue;
      if (el.getBoundingClientRect().height === 0) {
        bad.push(`${el.tagName}.${(el as HTMLElement).className || '(no class)'}`);
      }
    }
    return bad;
  });

  expect(zeroHeight, `${label}: visible elements with content must have height`).toEqual([]);

  const accent = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--mcpui-accent').trim(),
  );
  expect(accent, `${label}: --mcpui-accent resolves to the brand orange, got "${accent}"`).toMatch(ACCENT);
}

for (const entry of entries) {
  const url = 'file://' + path.join(GALLERY, entry.file);

  test.describe(`${entry.type} — light`, () => {
    // Why 1440 and not 1024: the widest real artifact — the nine-column CRM
    // table off the wire — overflows its own `overflow-x: auto` wrapper at
    // 1024, and a screenshot cannot show what has scrolled out of a container.
    // The result was a contact sheet of tables cut off mid-column. The 375px
    // shot below is where narrow behaviour is asserted; this pair exists to
    // show the whole artifact.
    test.use({ colorScheme: 'light', viewport: { width: 1440, height: 900 } });
    test(`renders`, async ({ page }) => {
      await page.goto(url);
      await assertRenderedSanely(page, `${entry.type} light`);
      await page.screenshot({ path: path.join(SHOTS, `${entry.type}-light.png`), fullPage: true });
    });
  });

  test.describe(`${entry.type} — dark`, () => {
    // The tokens are `light-dark()`, which resolves off the computed
    // `color-scheme` — which is exactly what this option sets.
    test.use({ colorScheme: 'dark', viewport: { width: 1440, height: 900 } });
    test(`renders`, async ({ page }) => {
      await page.goto(url);
      await assertRenderedSanely(page, `${entry.type} dark`);
      await page.screenshot({ path: path.join(SHOTS, `${entry.type}-dark.png`), fullPage: true });
    });
  });

  test.describe(`${entry.type} — narrow`, () => {
    test.use({ colorScheme: 'light', viewport: { width: 375, height: 812 } });
    test(`renders without horizontal overflow`, async ({ page }) => {
      await page.goto(url);
      await assertRenderedSanely(page, `${entry.type} narrow`);

      const overflow = await page.evaluate(() => ({
        scrollWidth: document.documentElement.scrollWidth,
        clientWidth: document.documentElement.clientWidth,
      }));
      expect(
        overflow.scrollWidth,
        `${entry.type}: the page must not scroll sideways at 375px (${overflow.scrollWidth} > ${overflow.clientWidth})`,
      ).toBeLessThanOrEqual(overflow.clientWidth);

      await page.screenshot({ path: path.join(SHOTS, `${entry.type}-narrow.png`), fullPage: true });
    });
  });
}
