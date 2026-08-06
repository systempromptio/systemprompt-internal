import { initCopyButtons } from './site/copy-buttons.js';
import { initStatusCard } from './site/status-card.js';

function initHeaderScroll() {
  const root = document.documentElement;
  const update = () => {
    if (window.scrollY > 24) root.setAttribute('data-scrolled', '');
    else root.removeAttribute('data-scrolled');
  };
  window.addEventListener('scroll', update, { passive: true });
  update();
}

function initHeroVideo() {
  const video = document.querySelector('.hero-bg-video');
  if (!video) return;
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    video.pause();
    return;
  }
  const conn = navigator.connection;
  if (conn && conn.saveData) {
    video.removeAttribute('autoplay');
    video.preload = 'none';
    return;
  }
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) video.pause();
    else video.play()?.catch?.(() => {});
  });
}

function initHeroVideoFade() {
  const video = document.querySelector('.hero-bg-video');
  if (!video) return;
  const show = () => video.classList.add('is-ready');
  if (video.readyState >= 2) show();
  else video.addEventListener('canplay', show, { once: true });
  setTimeout(show, 2500);
}

function initReveal() {
  const targets = document.querySelectorAll('[data-reveal]');
  if (!targets.length) return;

  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduced || !('IntersectionObserver' in window)) {
    targets.forEach((el) => el.classList.add('is-revealed'));
    return;
  }

  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) return;
      entry.target.classList.add('is-revealed');
      observer.unobserve(entry.target);
    });
  }, { rootMargin: '0px 0px -12% 0px', threshold: 0.1 });

  targets.forEach((el) => observer.observe(el));
}

export function initHomepage() {
  initCopyButtons();
  initStatusCard();
  initHeaderScroll();
  initHeroVideo();
  initHeroVideoFade();
  initReveal();
}

initHomepage();
