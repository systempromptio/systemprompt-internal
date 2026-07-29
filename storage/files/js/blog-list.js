const CARD_SELECTOR = 'a.content-card-link, a.blog-card-link';

function cardTitle(card) {
  return card.querySelector('.card-title')?.textContent?.toLowerCase() ?? '';
}

function cardDate(card) {
  return Date.parse(card.querySelector('.card-date, .meta-date')?.textContent ?? '') || 0;
}

const COMPARATORS = {
  'title-asc': (a, b) => cardTitle(a).localeCompare(cardTitle(b)),
  'title-desc': (a, b) => cardTitle(b).localeCompare(cardTitle(a)),
  'date-asc': (a, b) => cardDate(a) - cardDate(b),
  'date-desc': (a, b) => cardDate(b) - cardDate(a)
};

function applyFilter(grid, buttons, filter) {
  for (const button of buttons) button.classList.remove('active');

  const active = document.querySelector(`.blog-filter-btn[data-filter="${filter || 'all'}"]`);
  if (active) active.classList.add('active');
  grid.dataset.filter = filter || '';

  const url = new URL(window.location.href);
  if (filter && filter !== 'all') {
    url.searchParams.set('category', filter);
  } else {
    url.searchParams.delete('category');
  }
  window.history.replaceState({}, '', url);
}

function initSort(grid, select) {
  select.addEventListener('change', () => {
    const comparator = COMPARATORS[select.value];
    if (!comparator) return;

    const cards = Array.from(grid.querySelectorAll(CARD_SELECTOR));
    cards.sort(comparator);
    grid.append(...cards);
  });
}

export function initBlogList() {
  const grid = document.getElementById('blog-grid');
  if (!grid) return;

  const buttons = Array.from(document.querySelectorAll('.blog-filter-btn'));
  const initialFilter = new URLSearchParams(window.location.search).get('category');
  if (initialFilter) applyFilter(grid, buttons, initialFilter);

  for (const button of buttons) {
    button.addEventListener('click', () => {
      const filter = button.dataset.filter;
      applyFilter(grid, buttons, filter === 'all' ? '' : filter);
    });
  }

  const select = document.getElementById('blog-sort');
  if (select) initSort(grid, select);
}

initBlogList();
