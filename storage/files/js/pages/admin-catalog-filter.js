const search = document.getElementById('catalog-search');

if (search) {
  const targets = () => document.querySelectorAll('#catalog-grid .catalog-card, tr[data-search]');
  search.addEventListener('input', () => {
    const q = (search.value || '').toLowerCase();
    for (const el of targets()) {
      const haystack = el.getAttribute('data-search') || '';
      el.classList.toggle('is-filtered-out', Boolean(q) && !haystack.includes(q));
    }
  });
}
