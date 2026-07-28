import { apiFetch } from '../services/api.js';
import { showToast } from '../services/toast.js';
import {
  DB_SOURCE_LABEL,
  cloneTemplate,
  entitySections,
  setText,
  showPane,
  slot,
} from './admin-access-control-state.js';

const RULES_PATH = '/access-control';

const setRowSource = (select, label) => {
  const source = select.closest('.ac-dept-rule')?.querySelector('.ac-source');
  if (source) source.textContent = label;
};

const buildRule = (type, item) => {
  const row = cloneTemplate('ac-tpl-dept-rule');
  setText(row, 'label', item.label || item.id);
  setText(row, 'id', item.id);
  const select = row.querySelector('select');
  select.dataset.entityType = type;
  select.dataset.entityId = item.id;
  return row;
};

const buildSection = (section) => {
  const el = cloneTemplate('ac-tpl-dept-section');
  setText(el, 'label', section.label);
  const list = slot(el, 'rules');
  list.dataset.entityType = section.type;
  if (section.items.length === 0) {
    list.append(cloneTemplate('ac-tpl-dept-rule-empty'));
  } else {
    for (const item of section.items) list.append(buildRule(section.type, item));
  }
  return el;
};

const saveRule = (deptName, select) => {
  const { entityType, entityId } = select.dataset;
  if (select.value === 'inherit') {
    return apiFetch(`${RULES_PATH}/bulk-template`, {
      method: 'POST',
      body: JSON.stringify({
        entity_type: entityType,
        subject_type: 'department',
        subject_value: deptName,
        action: 'clear',
      }),
    });
  }
  const path = `${RULES_PATH}/entity/${encodeURIComponent(entityType)}/${encodeURIComponent(entityId)}/rules`;
  return apiFetch(path, {
    method: 'POST',
    body: JSON.stringify({ rule_type: 'department', rule_value: deptName, access: select.value }),
  });
};

const bindRules = (editor, deptName) => {
  for (const select of editor.querySelectorAll('select[data-action="dept-rule-access"]')) {
    select.addEventListener('change', async () => {
      try {
        await saveRule(deptName, select);
        setRowSource(select, select.value === 'inherit' ? '—' : DB_SOURCE_LABEL);
      } catch {
        showToast(`Could not save the rule for ${deptName}`, 'error');
      }
    });
  }
};

const applyCurrentRules = (editor, deptName, rules) => {
  for (const rule of rules) {
    if (rule.rule_type !== 'department' || rule.rule_value !== deptName) continue;
    const select = editor.querySelector(
      `select[data-entity-type="${CSS.escape(rule.entity_type)}"][data-entity-id="${CSS.escape(rule.entity_id)}"]`,
    );
    if (!select) continue;
    select.value = rule.access;
    setRowSource(select, DB_SOURCE_LABEL);
  }
};

const loadCurrentRules = async (editor, deptName) => {
  try {
    const resp = await apiFetch(RULES_PATH);
    applyCurrentRules(editor, deptName, resp?.rules || []);
  } catch {
    showToast('Could not load the current access rules', 'error');
  }
};

export const renderDeptEditor = (deptName) => {
  const editor = document.getElementById('ac-dept-editor');
  const content = cloneTemplate('ac-tpl-dept-editor');
  setText(content, 'name', deptName);
  const sections = slot(content, 'sections');
  if (deptName) {
    for (const section of entitySections()) sections.append(buildSection(section));
  } else {
    sections.append(cloneTemplate('ac-tpl-dept-unselected'));
  }
  editor.replaceChildren(content);
  showPane('ac-dept-editor');
  if (deptName) {
    bindRules(editor, deptName);
    loadCurrentRules(editor, deptName);
  }
};
