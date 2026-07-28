function createPaginationLink(link, direction, label) {
  const anchor = document.createElement('a');
  anchor.href = link.getAttribute('href');
  anchor.className = `docs-pagination-link docs-pagination-${direction}`;

  const labelSpan = document.createElement('span');
  labelSpan.className = 'docs-pagination-label';
  labelSpan.textContent = label;

  const titleSpan = document.createElement('span');
  titleSpan.className = 'docs-pagination-title';
  titleSpan.textContent = link.textContent.trim();

  anchor.append(labelSpan, titleSpan);
  return anchor;
}

function ensureNav() {
  const existing = document.querySelector('.docs-pagination');
  if (existing) return existing;

  const article = document.querySelector('.docs-article');
  if (!article) return null;

  const nav = document.createElement('nav');
  nav.className = 'docs-pagination';
  nav.setAttribute('aria-label', 'Pagination');
  article.append(nav);
  return nav;
}

export function initPagination() {
  const links = Array.from(document.querySelectorAll('.docs-sidebar .docs-nav-link'));
  if (!links.length) return;

  const currentPath = window.location.pathname;
  const currentIndex = links.findIndex((link) => link.getAttribute('href') === currentPath);
  if (currentIndex === -1) return;

  const nav = ensureNav();
  if (!nav) return;

  const fragment = document.createDocumentFragment();
  if (currentIndex > 0) {
    fragment.append(createPaginationLink(links[currentIndex - 1], 'prev', 'Previous'));
  }
  if (currentIndex < links.length - 1) {
    fragment.append(createPaginationLink(links[currentIndex + 1], 'next', 'Next'));
  }

  nav.replaceChildren(fragment);
}
