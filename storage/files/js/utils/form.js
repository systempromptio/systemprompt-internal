const toSelectedSet = (selected) => {
  const selectedSet = {};
  if (Array.isArray(selected)) {
    for (const s of selected) {
      const key = typeof s === 'string' ? s : (s.name || s.id || s);
      selectedSet[key] = true;
    }
  } else if (selected && typeof selected === 'object') {
    for (const k of Object.keys(selected)) {
      if (selected[k]) selectedSet[k] = true;
    }
  }
  return selectedSet;
};

const buildChecklistItem = (id, item, selectedSet) => {
  const val = typeof item === 'string' ? item : (item.name || item.id || item);
  const display = typeof item === 'string' ? item : (item.name || item.id || String(item));
  const itemId = id + '-chk-' + val.replace(/[^a-zA-Z0-9_-]/g, '_');

  const div = document.createElement('div');
  div.className = 'checklist-item';
  div.setAttribute('data-item-name', val.toLowerCase());

  const input = document.createElement('input');
  input.type = 'checkbox';
  input.name = id;
  input.value = val;
  input.id = itemId;
  if (selectedSet[val]) input.checked = true;

  const lbl = document.createElement('label');
  lbl.setAttribute('for', itemId);
  lbl.textContent = display;

  div.append(input, lbl);
  return div;
};

const buildEmptyState = () => {
  const empty = document.createElement('div');
  empty.className = 'empty-state checklist-empty';
  const p = document.createElement('p');
  p.textContent = 'None available.';
  empty.append(p);
  return empty;
};

const buildContainer = (id, items, selectedSet, hasSelectAll) => {
  const container = document.createElement('div');
  container.className = hasSelectAll ? 'checklist-container checklist-container--tall' : 'checklist-container';
  container.setAttribute('data-checklist', id);
  if (items?.length > 0) {
    for (const item of items) container.append(buildChecklistItem(id, item, selectedSet));
  } else {
    container.append(buildEmptyState());
  }
  return container;
};

const buildBulkButton = (id, attr, text) => {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'btn btn-secondary btn-sm';
  btn.setAttribute(attr, id);
  btn.textContent = text;
  return btn;
};

const buildFilterRow = (id, hasSelectAll) => {
  const filterInput = document.createElement('input');
  filterInput.type = 'text';
  filterInput.className = 'field-input checklist-filter';
  filterInput.placeholder = hasSelectAll ? 'Search...' : 'Filter...';
  filterInput.setAttribute('data-filter-list', id);
  if (!hasSelectAll) return filterInput;

  const filterRow = document.createElement('div');
  filterRow.className = 'checklist-filter-bar';
  filterRow.append(
    filterInput,
    buildBulkButton(id, 'data-select-all', 'Select All'),
    buildBulkButton(id, 'data-deselect-all', 'Deselect All')
  );
  return filterRow;
};

export const renderChecklist = (id, items, selected, label, opts = {}) => {
  const selectedSet = toSelectedSet(selected);
  const group = document.createElement('div');
  group.className = 'form-group';

  const labelEl = document.createElement('label');
  labelEl.className = 'field-label';
  labelEl.textContent = label;

  group.append(
    labelEl,
    buildFilterRow(id, Boolean(opts.hasSelectAll)),
    buildContainer(id, items, selectedSet, Boolean(opts.hasSelectAll))
  );
  return group;
};

export const attachFilterHandlers = (root) => {
  root.addEventListener('input', (e) => {
    const filterInput = e.target.closest('[data-filter-list]');
    if (!filterInput) return;
    const listId = filterInput.getAttribute('data-filter-list');
    const container = root.querySelector('[data-checklist="' + listId + '"]');
    if (!container) return;
    const q = filterInput.value.toLowerCase();
    for (const item of container.querySelectorAll('.checklist-item')) {
      const name = item.getAttribute('data-item-name') || '';
      item.hidden = Boolean(q) && !name.includes(q);
    }
  });
};

export const getCheckedValues = (form, name) => {
  const checked = form.querySelectorAll('input[name="' + name + '"]:checked');
  return Array.from(checked).map((cb) => cb.value);
};

export const formDataToObject = (formData) => {
  const obj = {};
  for (const [key, value] of formData.entries()) {
    if (key === 'tags') {
      obj[key] = value.split(',').map((t) => t.trim()).filter(Boolean);
    } else {
      obj[key] = value;
    }
  }
  return obj;
};
