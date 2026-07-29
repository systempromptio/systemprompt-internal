import { apiFetch } from '../services/api.js';
import { showToast } from '../services/toast.js';
import { on } from '../services/events.js';

const setupSnippet = ({ pat, origin }) => [
  '# 1. Install Pi and the gateway provider config on the target machine:',
  'examples/pi/setup.sh',
  '',
  '# 2. Point Pi at this token instead of a locally minted one:',
  'mkdir -p ~/.config/systemprompt-pi',
  `printf '%s' '${pat}' > ~/.config/systemprompt-pi/token`,
  `printf '%s' '${origin}' > ~/.config/systemprompt-pi/base-url`,
  'chmod 600 ~/.config/systemprompt-pi/token',
  '',
  '# 3. Verify the token reaches the gateway:',
  `curl -fsS -X POST '${origin}/api/public/gateway/sessions' -H 'x-api-key: ${pat}'`
].join('\n');

const openCreatePanel = () => {
  const overlay = document.getElementById('create-token-overlay');
  const panel = document.getElementById('create-token-panel');
  if (overlay && panel) {
    overlay.classList.add('open');
    panel.classList.add('open');
    const first = panel.querySelector('input, select');
    if (first) setTimeout(() => first.focus(), 350);
  }
};

const closeCreatePanel = () => {
  const overlay = document.getElementById('create-token-overlay');
  const panel = document.getElementById('create-token-panel');
  if (panel) panel.classList.remove('open');
  if (overlay) overlay.classList.remove('open');
};

const setPanelState = (state, ctx = {}) => {
  const formState = document.getElementById('new-token-form-state');
  const successState = document.getElementById('new-token-success-state');
  if (formState) formState.hidden = state !== 'form';
  if (successState) successState.hidden = state !== 'success';
  document.querySelectorAll('.panel-footer-state').forEach((el) => {
    el.hidden = el.getAttribute('data-state') !== state;
  });

  if (state === 'success') {
    const secretEl = document.getElementById('new-token-secret');
    if (secretEl) secretEl.value = ctx.secret || '';
    const snippetEl = document.getElementById('new-token-setup-snippet');
    if (snippetEl) {
      snippetEl.textContent = setupSnippet({
        pat: ctx.secret || '',
        origin: window.location.origin
      });
    }
  }
};

const resetForm = () => {
  for (const id of ['new-token-name', 'new-token-expires', 'new-token-secret']) {
    const el = document.getElementById(id);
    if (el) el.value = '';
  }
  const userSel = document.getElementById('new-token-user');
  if (userSel) userSel.value = '';
  const snippetEl = document.getElementById('new-token-setup-snippet');
  if (snippetEl) snippetEl.textContent = '';
  setPanelState('form');
};

const copyCurrentSnippet = async () => {
  const snippetEl = document.getElementById('new-token-setup-snippet');
  const text = snippetEl ? snippetEl.textContent : '';
  if (!text) { showToast('Nothing to copy yet', 'error'); return; }
  try {
    await navigator.clipboard.writeText(text);
    showToast('Setup snippet copied', 'success');
  } catch {
    showToast('Copy failed', 'error');
  }
};

const bindCreatePanel = () => {
  on('click', '#create-token-overlay', () => { closeCreatePanel(); });
  on('click', '#create-token-panel .panel-close', () => { closeCreatePanel(); });
  on('click', '#create-token-panel [data-action="cancel"]', () => {
    closeCreatePanel();
    resetForm();
  });
  on('click', '#create-token-panel [data-action="done"]', () => {
    closeCreatePanel();
    resetForm();
    window.location.reload();
  });
  on('click', '#create-token-panel [data-action="copy-snippet"]', () => {
    copyCurrentSnippet();
  });
  on('click', '#create-token-panel [data-action="save"]', async () => {
    const name = document.getElementById('new-token-name').value.trim();
    const userId = document.getElementById('new-token-user').value;
    const expiresAt = document.getElementById('new-token-expires').value.trim();
    if (!name) { showToast('Token name is required', 'error'); return; }
    if (!userId) { showToast('Owner is required', 'error'); return; }
    const body = expiresAt ? { name, expires_at: expiresAt } : { name };
    try {
      const result = await apiFetch(`/users/${encodeURIComponent(userId)}/pats`, {
        method: 'POST',
        body: JSON.stringify(body)
      });
      showToast('Access token issued', 'success');
      if (result && result.secret) {
        setPanelState('success', { secret: result.secret });
      } else {
        closeCreatePanel();
        resetForm();
        window.location.reload();
      }
    } catch (err) {
      showToast(err.message || 'Failed to issue token', 'error');
    }
  });
};

const bindSearch = () => {
  const search = document.getElementById('token-search');
  const apply = () => {
    const q = (search?.value || '').toLowerCase();
    for (const row of document.querySelectorAll('tr[data-search]')) {
      row.hidden = Boolean(q) && !row.dataset.search.includes(q);
    }
  };
  search?.addEventListener('input', apply);
};

export const initAccessTokensPage = () => {
  const page = document.querySelector('[data-page="tokens"]');
  if (page) {
    bindCreatePanel();
    bindSearch();
    const createBtn = document.querySelector('[data-action="create-token"]');
    if (createBtn) createBtn.addEventListener('click', openCreatePanel);
  }
};

initAccessTokensPage();
