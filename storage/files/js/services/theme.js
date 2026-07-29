import { safeStorageGet, safeStorageSet } from '../utils/storage-safe.js';

const STORAGE_KEY = 'sp-admin-theme';
const DARK = 'dark';
const LIGHT = 'light';

export const getPreferred = () => {
  const stored = safeStorageGet(STORAGE_KEY);
  if (stored === DARK || stored === LIGHT) return stored;
  if (window.matchMedia?.('(prefers-color-scheme: dark)').matches) return DARK;
  return LIGHT;
};

const updateToggle = (theme) => {
  const btn = document.getElementById('theme-toggle');
  if (!btn) return;
  const label = btn.querySelector('.theme-label');
  if (label) label.textContent = theme === DARK ? 'Light mode' : 'Dark mode';
  btn.setAttribute('aria-label', theme === DARK ? 'Switch to light mode' : 'Switch to dark mode');
};

const apply = (theme) => {
  document.documentElement.style.colorScheme = theme === DARK ? 'dark' : 'light';
  if (theme === DARK) {
    document.documentElement.setAttribute('data-theme', DARK);
  } else {
    document.documentElement.removeAttribute('data-theme');
  }
  updateToggle(theme);
};

const toggle = () => {
  const current = document.documentElement.getAttribute('data-theme');
  const next = current === DARK ? LIGHT : DARK;
  apply(next);
  safeStorageSet(STORAGE_KEY, next);
};

let themeReady = false;

export const initTheme = () => {
  if (themeReady) return;
  themeReady = true;
  window.matchMedia?.('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    if (!safeStorageGet(STORAGE_KEY)) apply(e.matches ? DARK : LIGHT);
  });

  document.getElementById('theme-toggle')?.addEventListener('click', toggle);
  apply(getPreferred());
};
