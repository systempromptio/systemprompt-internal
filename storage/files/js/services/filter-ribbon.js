// Identity filter ribbon — progressive enhancement over the SSR markup in
// partials/components/identity-filter-ribbon.hbs. The checkboxes and the
// submit button already work with JavaScript off; this adds the behaviour the
// markup has always described but nothing implemented: typeahead inside a
// group, one group open at a time, and dismissal by click-away or Escape.

const TYPEAHEAD_THRESHOLD = 10;

export const initFilterRibbon = () => {
  const ribbons = document.querySelectorAll('[data-filter-ribbon]');
  if (!ribbons.length) return;
  for (const ribbon of ribbons) {
    const groups = [...ribbon.querySelectorAll('details.filter-ribbon__group')];
    for (const group of groups) {
      initTypeahead(group);
      initExclusiveOpen(group, groups);
    }
    document.addEventListener('click', (e) => {
      if (!ribbon.contains(e.target)) closeAll(groups);
    });
    ribbon.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') return;
      const open = groups.find((g) => g.open);
      if (!open) return;
      open.open = false;
      open.querySelector('summary')?.focus();
    });
  }
};

// A search box over four options is noise, so it only survives on lists long
// enough to be worth filtering.
const initTypeahead = (group) => {
  const search = group.querySelector('[data-filter-typeahead]');
  const list = group.querySelector('[data-filter-list]');
  if (!search || !list) return;

  const items = [...list.querySelectorAll('.filter-ribbon__group-item')];
  if (items.length <= TYPEAHEAD_THRESHOLD) {
    search.hidden = true;
    return;
  }

  search.addEventListener('input', () => {
    const query = search.value.trim().toLowerCase();
    for (const item of items) {
      const label = (item.dataset.label || '').toLowerCase();
      item.hidden = query !== '' && !label.includes(query);
    }
  });
};

// Native <details> lets every group stand open at once, which stacks panels on
// top of each other. Opening one closes the rest.
const initExclusiveOpen = (group, groups) => {
  group.addEventListener('toggle', () => {
    if (!group.open) return;
    for (const other of groups) {
      if (other !== group) other.open = false;
    }
    const search = group.querySelector('[data-filter-typeahead]');
    if (search && !search.hidden) search.focus();
  });
};

const closeAll = (groups) => {
  for (const group of groups) group.open = false;
};
