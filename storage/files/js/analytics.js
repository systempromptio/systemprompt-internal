import { createState } from './site/analytics-state.js';
import { sendPageExit, sendPageView } from './site/analytics-transport.js';
import {
  handleBlur,
  handleClick,
  handleFocus,
  handleMouseMove,
  handleScroll,
  handleVisibilityChange,
  recordFirstInteraction
} from './site/analytics-handlers.js';

function throttle(fn, limit) {
  let timer = null;
  return (...args) => {
    if (timer) return;
    timer = setTimeout(() => {
      timer = null;
    }, limit);
    fn(...args);
  };
}

function bind(state) {
  const passive = { passive: true };
  const onScroll = throttle(() => handleScroll(state), 100);
  const onMouseMove = throttle((event) => handleMouseMove(state, event), 50);

  window.addEventListener('scroll', onScroll, passive);
  window.addEventListener('click', (event) => handleClick(state, event), passive);
  window.addEventListener('mousemove', onMouseMove, passive);
  window.addEventListener('keydown', () => {
    state.keyboardEvents += 1;
    recordFirstInteraction(state);
  }, passive);
  document.addEventListener('copy', () => {
    state.copyEvents += 1;
    recordFirstInteraction(state);
  });
  document.addEventListener('visibilitychange', () => handleVisibilityChange(state));
  window.addEventListener('focus', () => handleFocus(state));
  window.addEventListener('blur', () => handleBlur(state));
  window.addEventListener('pagehide', () => sendPageExit(state));
  window.addEventListener('beforeunload', () => sendPageExit(state));
}

export function initAnalytics() {
  if (window.__spAnalyticsInit) return;
  window.__spAnalyticsInit = true;

  const state = createState();
  sendPageView(state);
  bind(state);
  handleScroll(state);
}

initAnalytics();
