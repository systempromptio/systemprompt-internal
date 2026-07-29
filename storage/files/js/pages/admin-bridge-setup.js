const ARTIFACTS = {
  macos: { label: 'Download for macOS', file: 'systemprompt-bridge-macos.dmg' },
  windows: { label: 'Download for Windows', file: 'systemprompt-bridge-windows.exe' },
  linux: { label: 'Download for Linux', file: 'systemprompt-bridge-linux.tar.gz' }
};

const detectPlatform = (ua) => {
  if (/Mac/i.test(ua)) return 'macos';
  if (/Win/i.test(ua)) return 'windows';
  if (/Linux/i.test(ua)) return 'linux';
  return 'macos';
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

const tabs = document.querySelectorAll('.tabs button');
for (const btn of tabs) {
  btn.addEventListener('click', () => {
    for (const b of tabs) b.classList.toggle('active', b === btn);
    const target = btn.dataset.tab;
    document.getElementById('tab-tray').hidden = target !== 'tray';
    document.getElementById('tab-terminal').hidden = target !== 'terminal';
  });
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
wireCopy('toml-copy-btn', 'toml-snippet');
