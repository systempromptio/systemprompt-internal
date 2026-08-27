import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

// Drives core's real artifact shell the way a host does — the ui/initialize
// handshake, then a host-context notification — and asserts the artifact
// inside the nested sandboxed iframe repaints to the host's theme rather than
// the viewer's OS.
//
// The regression this locks down: the shell and the artifact both declare
// `color-scheme: light dark`, so on a dark-OS machine a LIGHT host showed a
// DARK artifact. Every assertion below therefore runs with colorScheme 'dark'
// emulated — if the host's word did not win, these would come back dark.

const SHELL = path.resolve(
  __dirname,
  '../../../systemprompt-core/crates/domain/mcp/src/services/ui_renderer/templates/assets/html/artifact-shell.html',
);
const GALLERY = path.resolve(__dirname, '../../target/artifact-gallery');
const ARTIFACT = path.join(GALLERY, 'table.html');

const ready = fs.existsSync(SHELL) && fs.existsSync(ARTIFACT);
test.skip(!ready, 'needs the sibling core checkout and `just artifact-gallery`');

// The generated MCP_UI constants are injected server-side via a placeholder,
// so the raw template has none. Substitute the two method names this flow
// needs, taken from core's `UiMethod` enum.
function shellHtml(): string {
  return fs.readFileSync(SHELL, 'utf8').replace(
    '/*MCP_UI_CONSTANTS*/',
    `const MCP_UI = Object.freeze({
       INITIALIZE: 'ui/initialize',
       INITIALIZED: 'ui/notifications/initialized',
       TOOL_RESULT: 'ui/notifications/tool-result',
       SIZE_CHANGED: 'ui/notifications/size-changed',
       HOST_CONTEXT_CHANGED: 'ui/notifications/host-context-changed',
       PROTOCOL_VERSION: '2026-01-26'
     });`,
  );
}

// Why not a bare JSON.stringify: both the shell and the artifact contain
// `</script>`, and `page.setContent` writes this markup into the document —
// so an unescaped closer breaks out of the host's own script tag and nothing
// mounts at all.
function jsLiteral(value: string): string {
  return JSON.stringify(value).replace(/<\//g, '<\\/');
}

// A minimal host page: embeds the shell, answers ui/initialize with the theme
// it was built for, and forwards the artifact as an embedded resource.
function hostHtml(shell: string, artifact: string, theme: string | null): string {
  const initialContext = theme ? `hostContext: { theme: ${JSON.stringify(theme)} }` : '';
  return `<!doctype html><meta charset="utf-8">
<iframe id="shell" style="width:100%;height:600px;border:0"></iframe>
<script>
  const ARTIFACT = ${jsLiteral(artifact)};
  const shell = document.getElementById('shell');
  window.addEventListener('message', (e) => {
    const d = e.data || {};
    if (d.method === 'ui/initialize' && d.id) {
      shell.contentWindow.postMessage({ jsonrpc:'2.0', id:d.id, result:{ ${initialContext} } }, '*');
      shell.contentWindow.postMessage({ jsonrpc:'2.0', method:'ui/notifications/tool-result',
        params:{ content:[{ type:'resource', resource:{ uri:'ui://t/artifact/1',
          mimeType:'text/html;profile=mcp-app', text:ARTIFACT } }] } }, '*');
    }
  });
  window.setTheme = (theme) => shell.contentWindow.postMessage({ jsonrpc:'2.0',
    method:'ui/notifications/host-context-changed', params:{ hostContext:{ theme } } }, '*');
  shell.srcdoc = ${jsLiteral(shell)};
</script>`;
}

// The artifact lives two frames down: host -> shell -> srcdoc artifact.
// Walk the tree explicitly rather than scanning by URL — both nested frames
// are `about:srcdoc`, and picking the first match silently selects the SHELL,
// whose documentElement this fix stamps directly. That made an earlier version
// of this test pass with the frame.js half of the fix deleted.
function artifactFrame(page: import('@playwright/test').Page) {
  const shell = page.mainFrame().childFrames()[0];
  expect(shell, 'the shell iframe is present').toBeTruthy();
  const artifact = shell?.childFrames()[0];
  expect(artifact, 'the artifact iframe is mounted inside the shell').toBeTruthy();
  return artifact!;
}

async function artifactScheme(page: import('@playwright/test').Page): Promise<string> {
  return artifactFrame(page).evaluate(
    () => getComputedStyle(document.documentElement).colorScheme,
  );
}

test.describe('host theme wins over the OS', () => {
  // Emulate a DARK operating system throughout: this is the condition under
  // which the bug appeared.
  test.use({ colorScheme: 'dark' });

  test('a light host renders a light artifact on a dark OS', async ({ page }) => {
    await page.setContent(hostHtml(shellHtml(), fs.readFileSync(ARTIFACT, 'utf8'), 'light'));
    await expect.poll(() => artifactScheme(page), { timeout: 5000 }).toBe('light');

    const bg = await artifactFrame(page).evaluate(
      () => getComputedStyle(document.body).backgroundColor,
    );
    expect(bg, 'a light artifact paints a light ground').not.toBe('rgba(0, 0, 0, 0)');
  });

  test('a dark host still renders a dark artifact', async ({ page }) => {
    await page.setContent(hostHtml(shellHtml(), fs.readFileSync(ARTIFACT, 'utf8'), 'dark'));
    await expect.poll(() => artifactScheme(page), { timeout: 5000 }).toBe('dark');
  });

  test('a host that says nothing falls back to the OS', async ({ page }) => {
    await page.setContent(hostHtml(shellHtml(), fs.readFileSync(ARTIFACT, 'utf8'), null));
    // `color-scheme: light dark` under a dark OS resolves dark — unchanged
    // behaviour for any host that does not report a theme.
    await expect.poll(() => artifactScheme(page), { timeout: 5000 }).toBe('light dark');
  });

  test('a theme change after mount repaints the artifact', async ({ page }) => {
    await page.setContent(hostHtml(shellHtml(), fs.readFileSync(ARTIFACT, 'utf8'), 'dark'));
    await expect.poll(() => artifactScheme(page), { timeout: 5000 }).toBe('dark');
    await page.evaluate(() => (window as any).setTheme('light'));
    await expect.poll(() => artifactScheme(page), { timeout: 5000 }).toBe('light');
  });
});
