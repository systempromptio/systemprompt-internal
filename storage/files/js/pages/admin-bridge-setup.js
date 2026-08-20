// Only platforms with a binary staged in storage/files/downloads/ are listed;
// macOS and Linux aarch64 are not currently hosted.
const ARTIFACTS = {
  windows: { label: 'Download for Windows', file: 'systemprompt-internal-bridge-windows.exe' },
  'linux-x86_64': { label: 'Download for Linux (x86_64)', file: 'systemprompt-internal-bridge-linux-x86_64.tar.gz' }
};

const detectPlatform = (ua) => {
  if (/Linux|Android/i.test(ua) && !/aarch64|arm64|armv8/i.test(ua)) return 'linux-x86_64';
  return 'windows';
};

const pill = document.getElementById('gateway-pill');
const gateway = pill?.dataset.gatewayUrl || '';
const downloadBase = pill?.dataset.downloadBase || '';

const cta = document.getElementById('download-cta');
if (cta) {
  const artifact = ARTIFACTS[detectPlatform(navigator.userAgent)];
  cta.href = `${downloadBase}/${artifact.file}`;
  cta.textContent = artifact.label;
}

if (pill) {
  fetch(`${gateway}/v1/auth/bridge/capabilities`)
    .then((r) => {
      pill.className = r.ok ? 'pill ok' : 'pill err';
      pill.textContent = r.ok ? 'Gateway reachable' : `Gateway error ${r.status}`;
    })
    .catch(() => {
      pill.className = 'pill err';
      pill.textContent = 'Gateway unreachable';
    });
}

const tabs = [...document.querySelectorAll('.tabs button')];
const selectTab = (name) => {
  for (const b of tabs) b.classList.toggle('active', b.dataset.tab === name);
  for (const b of tabs) {
    const panel = document.getElementById(`tab-${b.dataset.tab}`);
    if (panel) panel.hidden = b.dataset.tab !== name;
  }
};
for (const btn of tabs) btn.addEventListener('click', () => selectTab(btn.dataset.tab));

if (detectPlatform(navigator.userAgent).startsWith('linux') && tabs.some((b) => b.dataset.tab === 'linux')) {
  selectTab('linux');
}

const wireCopy = (btnId, srcId) => {
  const btn = document.getElementById(btnId);
  const src = document.getElementById(srcId);
  if (!btn || !src) return;
  btn.addEventListener('click', () => {
    navigator.clipboard.writeText(src.innerText).then(() => {
      btn.textContent = 'Copied';
      setTimeout(() => {
        btn.textContent = 'Copy';
      }, 1500);
    });
  });
};

wireCopy('cli-copy-btn', 'cli-snippet');
wireCopy('linux-copy-btn', 'linux-snippet');
wireCopy('toml-copy-btn', 'toml-snippet');
