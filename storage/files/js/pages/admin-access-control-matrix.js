import { apiFetch } from '../services/api.js';
import { showToast } from '../services/toast.js';
import {
  DEFAULT_DEPARTMENT,
  cloneTemplate,
  departmentNames,
  setText,
  showPane,
  slot,
} from './admin-access-control-state.js';

const LAYER_CLASSES = {
  user: 'is-user',
  department: 'is-department',
  role: 'is-role',
  default: 'is-default',
};

const setOverrideState = (row, layer, effective) => {
  const pressed = {
    allow: layer === 'user' && effective === 'allow',
    deny: layer === 'user' && effective === 'deny',
    inherit: layer !== 'user',
  };
  for (const btn of row.querySelectorAll('.ac-override button')) {
    btn.setAttribute('aria-pressed', String(pressed[btn.dataset.override]));
  }
};

const buildRow = (entityType, data) => {
  const effective = data.effective || 'deny';
  const row = cloneTemplate('ac-tpl-matrix-row');
  row.dataset.entityType = entityType;
  row.dataset.entityId = data.entity_id;
  setText(row, 'name', data.entity_name);
  setText(row, 'id', data.entity_id);
  setText(row, 'effective', effective);
  slot(row, 'effective').classList.add(`is-${effective}`);
  setText(row, 'layer', data.source.layer);
  slot(row, 'layer').classList.add(LAYER_CLASSES[data.source.layer] || 'is-default');
  setText(row, 'detail', data.source.detail);
  setOverrideState(row, data.source.layer, effective);
  return row;
};

const buildSection = (section) => {
  const rows = section.rows || [];
  const el = cloneTemplate('ac-tpl-matrix-section');
  setText(el, 'label', section.label);
  setText(el, 'count', rows.length === 0 ? '' : rows.length);
  if (rows.length === 0) {
    el.append(cloneTemplate('ac-tpl-matrix-empty'));
  } else {
    for (const row of rows) el.append(buildRow(section.entity_type, row));
  }
  return el;
};

const buildDeptSelect = (select, current) => {
  for (const name of departmentNames()) {
    const option = document.createElement('option');
    option.value = name;
    option.textContent = name;
    option.selected = name === current;
    select.append(option);
  }
};

const buildHeader = (content, user) => {
  setText(content, 'name', user.display_name || user.email || user.id);
  setText(content, 'email', user.email || '');
  setText(content, 'roles', `roles: ${(user.roles || []).join(', ') || 'none'}`);
};

const saveOverride = (userId, btn) => {
  const cell = btn.closest('.ac-override');
  const { entityType, entityId } = cell.closest('.ac-matrix-row').dataset;
  if (btn.dataset.override === 'inherit') {
    return apiFetch('/access-control/bulk-template', {
      method: 'POST',
      body: JSON.stringify({
        entity_type: entityType,
        subject_type: 'user',
        subject_value: userId,
        action: 'clear',
      }),
    });
  }
  const path = `/access-control/entity/${encodeURIComponent(entityType)}/${encodeURIComponent(entityId)}/rules`;
  return apiFetch(path, {
    method: 'POST',
    body: JSON.stringify({ rule_type: 'user', rule_value: userId, access: btn.dataset.override }),
  });
};

const bindOverrides = (matrix, userId, displayName) => {
  for (const btn of matrix.querySelectorAll('.ac-override button')) {
    btn.addEventListener('click', async () => {
      try {
        await saveOverride(userId, btn);
        renderUserMatrix(userId, displayName);
      } catch {
        showToast('Could not save the override', 'error');
      }
    });
  }
};

const bindDeptSelect = (matrix, userId, displayName, current) => {
  const select = matrix.querySelector('select[data-action="assign-dept"]');
  select.addEventListener('change', async () => {
    try {
      await apiFetch(`/management/users/${encodeURIComponent(userId)}/department`, {
        method: 'PUT',
        body: JSON.stringify({ department_name: select.value }),
      });
      renderUserMatrix(userId, displayName);
    } catch {
      showToast('Could not assign the department', 'error');
      select.value = current;
    }
  });
};

export const renderUserMatrix = async (userId, displayName) => {
  const matrix = document.getElementById('ac-user-matrix');
  const loading = cloneTemplate('ac-tpl-matrix-loading');
  setText(loading, 'name', displayName);
  matrix.replaceChildren(loading);
  showPane('ac-user-matrix');

  let data;
  try {
    data = await apiFetch(`/access-control/users/${encodeURIComponent(userId)}/matrix`);
  } catch {
    matrix.replaceChildren(cloneTemplate('ac-tpl-matrix-error'));
    return;
  }

  const content = cloneTemplate('ac-tpl-matrix');
  buildHeader(content, data.user);
  const current = data.user.department || DEFAULT_DEPARTMENT;
  buildDeptSelect(content.querySelector('select[data-action="assign-dept"]'), current);
  const sections = slot(content, 'sections');
  for (const section of data.sections || []) sections.append(buildSection(section));
  matrix.replaceChildren(content);
  bindDeptSelect(matrix, userId, displayName, current);
  bindOverrides(matrix, userId, displayName);
};
