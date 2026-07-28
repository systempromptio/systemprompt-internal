import { SCROLL_MILESTONES } from './analytics-state.js';
import { detectRageClick, getScrollDepth } from './analytics-metrics.js';
import { sendLinkClick, sendPageExit, sendScrollMilestone } from './analytics-transport.js';

const INTERACTIVE_TAGS = new Set(['A', 'BUTTON', 'INPUT']);

export function recordFirstInteraction(state) {
  if (!state.firstInteractionTime) {
    state.firstInteractionTime = Date.now();
  }
}

function trackMilestones(state, depth) {
  for (const milestone of SCROLL_MILESTONES) {
    if (depth >= milestone && !state.scrollMilestonesSent[milestone]) {
      sendScrollMilestone(state, milestone);
    }
  }
}

function trackDirection(state, position) {
  if (state.scrollPositions.length === 0) return;

  const last = state.scrollPositions[state.scrollPositions.length - 1].position;
  const direction = position > last ? 'down' : 'up';

  if (state.lastScrollDirection && direction !== state.lastScrollDirection) {
    state.scrollDirectionChanges += 1;
  }

  state.lastScrollDirection = direction;
}

export function handleScroll(state) {
  const depth = getScrollDepth();
  const position = window.scrollY;
  const time = Date.now();

  if (!state.firstScrollTime) {
    state.firstScrollTime = time;
    recordFirstInteraction(state);
  }

  if (depth > state.maxScrollDepth) {
    state.maxScrollDepth = depth;
    trackMilestones(state, depth);
  }

  trackDirection(state, position);
  state.scrollPositions.push({ position, time });

  if (state.scrollPositions.length > 50) {
    state.scrollPositions = state.scrollPositions.slice(-50);
  }
}

export function handleClick(state, event) {
  state.clickCount += 1;
  recordFirstInteraction(state);
  detectRageClick(state, Date.now());

  const target = event.target;
  if (!(target instanceof Element)) return;

  const link = target.closest('a');
  if (link && link.href) {
    sendLinkClick(link.href, link.textContent, link.hostname !== window.location.hostname);
  }

  const isInteractive = INTERACTIVE_TAGS.has(target.tagName) || link || target.closest('button');
  if (!isInteractive && state.clickCount > 1) {
    state.hasDeadClick = true;
  }
}

export function handleMouseMove(state, event) {
  if (state.lastMousePosition) {
    const dx = event.clientX - state.lastMousePosition.x;
    const dy = event.clientY - state.lastMousePosition.y;
    state.mouseDistance += Math.sqrt(dx * dx + dy * dy);
  }

  state.lastMousePosition = { x: event.clientX, y: event.clientY };
}

export function handleVisibilityChange(state) {
  const now = Date.now();
  const elapsed = now - state.lastVisibilityChange;

  if (state.isVisible) {
    state.visibleTime += elapsed;
    state.focusTime += elapsed;
  } else {
    state.hiddenTime += elapsed;
  }

  state.isVisible = !document.hidden;
  state.lastVisibilityChange = now;

  if (document.hidden) {
    state.tabSwitches += 1;
    sendPageExit(state);
  }
}

export function handleFocus(state) {
  if (state.isVisible) return;

  const now = Date.now();
  state.hiddenTime += now - state.lastVisibilityChange;
  state.lastVisibilityChange = now;
  state.isVisible = true;
}

export function handleBlur(state) {
  state.blurCount += 1;
  if (!state.isVisible) return;

  const now = Date.now();
  const elapsed = now - state.lastVisibilityChange;
  state.visibleTime += elapsed;
  state.focusTime += elapsed;
  state.lastVisibilityChange = now;
  state.isVisible = false;
}
