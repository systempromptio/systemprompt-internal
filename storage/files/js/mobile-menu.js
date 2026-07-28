function registerOverlayEscape() {
  window._overlays = window._overlays || [];

  if (!window._overlayEscapeInit) {
    window._overlayEscapeInit = true;
    document.addEventListener('keydown', (event) => {
      if (event.key !== 'Escape') return;
      for (let i = window._overlays.length - 1; i >= 0; i -= 1) {
        if (window._overlays[i]()) break;
      }
    });
  }

  return window._overlays;
}

export function initMobileMenu() {
  const menuToggle = document.querySelector('.mobile-menu-toggle');
  if (!menuToggle || menuToggle.dataset.menuBound === 'true') return;
  menuToggle.dataset.menuBound = 'true';

  const navLinks = document.querySelector('.nav-links');
  const docsSidebar = document.querySelector('.docs-sidebar');
  const panel = docsSidebar ?? navLinks;

  const closeMenu = () => {
    panel?.classList.remove('is-open');
    menuToggle.setAttribute('aria-expanded', 'false');
    document.body.classList.remove('menu-open');
  };

  menuToggle.addEventListener('click', () => {
    const expanded = menuToggle.getAttribute('aria-expanded') === 'true';
    menuToggle.setAttribute('aria-expanded', String(!expanded));
    panel?.classList.toggle('is-open');
    document.body.classList.toggle('menu-open');
  });

  if (panel) {
    for (const link of panel.querySelectorAll('a')) {
      link.addEventListener('click', closeMenu);
    }
  }

  registerOverlayEscape().push(() => {
    if (!document.body.classList.contains('menu-open')) return false;
    closeMenu();
    return true;
  });
}

initMobileMenu();
