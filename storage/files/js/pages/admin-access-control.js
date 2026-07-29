import { loadState, showPane } from './admin-access-control-state.js';
import { renderDeptEditor } from './admin-access-control-editors.js';
import { renderUserMatrix } from './admin-access-control-matrix.js';
import { bindModals } from './admin-access-control-modals.js';

const layout = () => document.querySelector('.ac-layout');

const clearActive = (root) => {
  for (const btn of root.querySelectorAll('[aria-pressed="true"]')) {
    btn.setAttribute('aria-pressed', 'false');
  }
};

const selectTarget = (root, target) => {
  clearActive(root);
  target.setAttribute('aria-pressed', 'true');
  if (target.dataset.action === 'select-dept') {
    renderDeptEditor(target.dataset.dept || '');
  } else {
    renderUserMatrix(target.dataset.userId, target.dataset.userDisplay);
  }
};

const bindTree = (root) => {
  root.addEventListener('click', (ev) => {
    const target = ev.target.closest('[data-action="select-dept"], [data-action="select-user"]');
    if (!target) return;
    ev.preventDefault();
    selectTarget(root, target);
  });
};

const bindSearch = (root) => {
  const input = document.getElementById('ac-search');
  input.addEventListener('input', () => {
    const q = input.value.trim().toLowerCase();
    for (const dept of root.querySelectorAll('.ac-tree-dept')) {
      let anyMatch = false;
      for (const row of dept.querySelectorAll('.ac-user-row')) {
        const name = (row.dataset.userDisplay || '').toLowerCase();
        const email = (row.dataset.userEmail || '').toLowerCase();
        const visible = !q || name.includes(q) || email.includes(q);
        row.parentElement.hidden = !visible;
        if (visible) anyMatch = true;
      }
      dept.hidden = Boolean(q) && !anyMatch && !(dept.dataset.dept || '').toLowerCase().includes(q);
    }
  });
};

const focusRequestedUser = (root) => {
  const userId = new URLSearchParams(window.location.search).get('user');
  const target = userId
    ? root.querySelector(`.ac-user-row[data-user-id="${CSS.escape(userId)}"]`)
    : null;
  if (target) {
    selectTarget(root, target);
  } else {
    showPane('ac-welcome');
  }
};

const init = () => {
  const root = layout();
  if (!root) return;
  loadState();
  bindTree(root);
  bindSearch(root);
  bindModals();
  focusRequestedUser(root);
};

init();
