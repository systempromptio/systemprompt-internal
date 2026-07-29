import { apiFetch } from '../services/api.js';
import { showToast } from '../services/toast.js';
import { departmentNames } from './admin-access-control-state.js';

const MODAL_IDS = ['ac-yaml-modal', 'ac-new-dept-modal'];

const closeModals = () => {
  document.getElementById('ac-modal-overlay').hidden = true;
  for (const id of MODAL_IDS) document.getElementById(id).hidden = true;
};

const openModal = (id) => {
  document.getElementById('ac-modal-overlay').hidden = false;
  const modal = document.getElementById(id);
  modal.hidden = false;
  modal.focus();
};

const showDeptError = (message) => {
  const err = document.getElementById('ac-new-dept-error');
  err.textContent = message;
  err.hidden = !message;
};

const showYamlModal = async () => {
  openModal('ac-yaml-modal');
  const target = document.getElementById('ac-yaml-content');
  target.textContent = 'Loading…';
  try {
    const yaml = await apiFetch('/access-control/yaml-snapshot');
    target.textContent = yaml || '# (no role/department rules in DB yet)\n';
  } catch {
    target.textContent = '# Failed to load the YAML snapshot.';
  }
};

const copyYaml = async () => {
  const button = document.getElementById('ac-yaml-copy');
  try {
    await navigator.clipboard.writeText(document.getElementById('ac-yaml-content').textContent);
    button.textContent = 'Copied!';
    setTimeout(() => { button.textContent = 'Copy'; }, 1500);
  } catch {
    showToast('Copy failed — select the text manually', 'error');
  }
};

const saveNewDepartment = async () => {
  const name = document.getElementById('ac-new-dept-name').value.trim();
  const description = document.getElementById('ac-new-dept-desc').value.trim();
  showDeptError('');
  if (!name) {
    showDeptError('Name is required');
    return;
  }
  if (departmentNames().some((n) => n.toLowerCase() === name.toLowerCase())) {
    showDeptError('A department with that name already exists.');
    return;
  }
  try {
    await apiFetch('/management/departments', {
      method: 'POST',
      body: JSON.stringify({ name, description }),
    });
    window.location.reload();
  } catch (err) {
    showDeptError(err.message || 'Failed to create department');
  }
};

const openNewDeptModal = () => {
  document.getElementById('ac-new-dept-name').value = '';
  document.getElementById('ac-new-dept-desc').value = '';
  showDeptError('');
  openModal('ac-new-dept-modal');
};

export const bindModals = () => {
  document.getElementById('ac-show-yaml').addEventListener('click', showYamlModal);
  document.getElementById('ac-new-department').addEventListener('click', openNewDeptModal);
  document.getElementById('ac-yaml-copy').addEventListener('click', copyYaml);
  document.getElementById('ac-new-dept-save').addEventListener('click', saveNewDepartment);
  document.getElementById('ac-modal-overlay').addEventListener('click', closeModals);
  for (const id of ['ac-yaml-close', 'ac-new-dept-close', 'ac-new-dept-cancel']) {
    document.getElementById(id).addEventListener('click', closeModals);
  }
  for (const id of MODAL_IDS) {
    document.getElementById(id).addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') closeModals();
    });
  }
};
