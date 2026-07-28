const ANCHOR_OFFSET = 80;

export function initNavActiveState() {
  const currentPath = window.location.pathname;

  for (const link of document.querySelectorAll('.docs-nav-link')) {
    if (link.getAttribute('href') !== currentPath) continue;
    link.classList.add('docs-nav-link--active');
    const details = link.closest('details');
    if (details) details.open = true;
  }
}

export function initCollapsibleNav() {
  const currentPath = window.location.pathname;

  for (const detail of document.querySelectorAll('.docs-nav-details')) {
    for (const link of detail.querySelectorAll('.docs-nav-link')) {
      if (link.getAttribute('href') === currentPath) detail.open = true;
    }
  }
}

export function initSmoothScroll() {
  for (const anchor of document.querySelectorAll('a[href^="#"]')) {
    anchor.addEventListener('click', (event) => {
      const href = anchor.getAttribute('href');
      if (href === '#') return;

      const target = document.getElementById(href.slice(1));
      if (!target) return;

      event.preventDefault();
      window.scrollTo({ top: target.offsetTop - ANCHOR_OFFSET, behavior: 'smooth' });
      history.pushState(null, '', href);
    });
  }
}
