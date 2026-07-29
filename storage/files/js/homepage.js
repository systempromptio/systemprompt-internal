import { initCopyButtons } from './site/copy-buttons.js';
import { initStatusCard } from './site/status-card.js';

function initHeaderScroll() {
  const header = document.querySelector('.site-header');
  if (!header) return;
  const update = () => header.classList.toggle('is-scrolled', window.scrollY > 24);
  window.addEventListener('scroll', update, { passive: true });
  update();
}

function initHeroVideo() {
  const video = document.querySelector('.hero-bg-video');
  if (video && window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    video.pause();
  }
}

export function initHomepage() {
  initCopyButtons();
  initStatusCard();
  initHeaderScroll();
  initHeroVideo();
}

initHomepage();
