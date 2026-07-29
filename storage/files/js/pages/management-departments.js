import { rawFetch } from '../services/api.js';
import { showConfirmDialog } from '../services/confirm.js';

const DEPARTMENTS_URL = '/admin/management/departments';

const showInlineError = (el, err) => {
  el.textContent = err.message;
  el.hidden = false;
};

const bindCreateDialog = () => {
  const dlg = document.getElementById('dept-create-dialog');
  const form = document.getElementById('dept-create-form');
  const errEl = document.getElementById('dept-create-error');
  const open = () => {
    errEl.hidden = true;
    form.reset();
    dlg.showModal();
  };
  document.getElementById('btn-new-department')?.addEventListener('click', open);
  document.getElementById('btn-new-department-empty')?.addEventListener('click', open);
  document.getElementById('dept-create-cancel')?.addEventListener('click', () => dlg.close());
  form?.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const fd = new FormData(form);
    try {
      await rawFetch(DEPARTMENTS_URL, {
        method: 'POST',
        body: JSON.stringify({ name: fd.get('name'), description: fd.get('description') || '' }),
      });
      location.reload();
    } catch (err) {
      showInlineError(errEl, err);
    }
  });
};

const bindDeleteButtons = () => {
  for (const btn of document.querySelectorAll('[data-delete-dept]')) {
    btn.addEventListener('click', (ev) => {
      ev.stopPropagation();
      const id = btn.dataset.deleteDept;
      const name = btn.dataset.deptName;
      showConfirmDialog(
        `Delete department "${name}"?`,
        'Members will be reassigned to "Default" and department-level access rules removed.',
        'Delete',
        async () => {
          await rawFetch(`${DEPARTMENTS_URL}/${id}`, { method: 'DELETE' });
          location.reload();
        },
      );
    });
  }
};

const bindSearch = () => {
  document.getElementById('dept-search')?.addEventListener('input', (ev) => {
    const q = ev.target.value.toLowerCase();
    for (const row of document.querySelectorAll('[data-dept-name]')) {
      row.hidden = Boolean(q) && !row.dataset.deptName.includes(q);
    }
  });
};

bindCreateDialog();
bindDeleteButtons();
bindSearch();
