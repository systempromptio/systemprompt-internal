import { rawFetch } from '../services/api.js';
import { showConfirmDialog } from '../services/confirm.js';

const DEPARTMENTS_URL = '/admin/management/departments';
const USERS_URL = '/admin/management/users';

const pageEl = document.querySelector('[data-dept-id]');
const deptId = pageEl?.dataset.deptId;
const deptName = pageEl?.dataset.deptName;

const setUserDepartment = (userId, departmentName) =>
  rawFetch(`${USERS_URL}/${encodeURIComponent(userId)}/department`, {
    method: 'PUT',
    body: JSON.stringify({ department_name: departmentName }),
  });

const bindAddMember = () => {
  const dlg = document.getElementById('member-add-dialog');
  const errEl = document.getElementById('member-add-error');
  const open = () => {
    errEl.hidden = true;
    dlg?.showModal();
  };
  document.getElementById('btn-add-member')?.addEventListener('click', open);
  document.getElementById('btn-add-member-empty')?.addEventListener('click', open);
  document.getElementById('member-add-cancel')?.addEventListener('click', () => dlg.close());
  document.getElementById('member-add-form')?.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const userId = String(new FormData(ev.target).get('user_id') || '').trim();
    if (userId) {
      try {
        await setUserDepartment(userId, deptName);
        location.reload();
      } catch (err) {
        errEl.textContent = err.message;
        errEl.hidden = false;
      }
    }
  });
};

const bindUnassignButtons = () => {
  for (const btn of document.querySelectorAll('[data-unassign-user]')) {
    btn.addEventListener('click', () => {
      showConfirmDialog('Move member?', 'Move this user back to "Default"?', 'Move', async () => {
        await setUserDepartment(btn.dataset.unassignUser, 'Default');
        location.reload();
      });
    });
  }
};

const bindSettings = () => {
  document.getElementById('dept-settings-form')?.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const fd = new FormData(ev.target);
    const msg = document.getElementById('dept-settings-msg');
    try {
      await rawFetch(`${DEPARTMENTS_URL}/${deptId}`, {
        method: 'PUT',
        body: JSON.stringify({ name: fd.get('name'), description: fd.get('description') || '' }),
      });
      msg.textContent = 'Saved.';
      msg.hidden = false;
      setTimeout(() => location.reload(), 600);
    } catch (err) {
      msg.textContent = `Save failed: ${err.message}`;
      msg.hidden = false;
    }
  });
  document.getElementById('dept-delete-btn')?.addEventListener('click', () => {
    showConfirmDialog(
      `Delete "${deptName}"?`,
      'Members will be reassigned to "Default" and department-level access rules removed.',
      'Delete',
      async () => {
        await rawFetch(`${DEPARTMENTS_URL}/${deptId}`, { method: 'DELETE' });
        location.assign('/admin/access/departments');
      },
    );
  });
};

const bindSearch = () => {
  document.getElementById('member-search')?.addEventListener('input', (ev) => {
    const q = ev.target.value.toLowerCase();
    for (const row of document.querySelectorAll('[data-member-email]')) {
      row.hidden = Boolean(q) && !row.dataset.memberEmail.includes(q);
    }
  });
};

bindAddMember();
bindUnassignButtons();
bindSettings();
bindSearch();
