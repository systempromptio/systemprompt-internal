import { RAGE_CLICK_THRESHOLD, RAGE_CLICK_WINDOW_MS } from './analytics-state.js';

export function getScrollDepth() {
  const windowHeight = window.innerHeight;
  const documentHeight = Math.max(
    document.body.scrollHeight,
    document.body.offsetHeight,
    document.documentElement.scrollHeight,
    document.documentElement.offsetHeight
  );
  const scrollTop = window.scrollY || document.documentElement.scrollTop;

  if (documentHeight <= windowHeight) {
    return 100;
  }

  return Math.min(100, Math.round(((scrollTop + windowHeight) / documentHeight) * 100));
}

export function calculateScrollVelocity(state) {
  if (state.scrollPositions.length < 2) {
    return null;
  }

  const recent = state.scrollPositions.slice(-10);
  let totalVelocity = 0;

  for (let i = 1; i < recent.length; i += 1) {
    const timeDiff = recent[i].time - recent[i - 1].time;
    const posDiff = Math.abs(recent[i].position - recent[i - 1].position);
    if (timeDiff > 0) {
      totalVelocity += posDiff / timeDiff;
    }
  }

  return Math.round((totalVelocity / (recent.length - 1)) * 1000);
}

export function detectRageClick(state, timestamp) {
  state.clickTimestamps.push(timestamp);
  state.clickTimestamps = state.clickTimestamps.filter((t) => timestamp - t < RAGE_CLICK_WINDOW_MS);

  if (state.clickTimestamps.length >= RAGE_CLICK_THRESHOLD) {
    state.hasRageClick = true;
  }
}

export function detectReadingPattern(state) {
  const timeOnPage = Date.now() - state.pageLoadTime;
  const depth = state.maxScrollDepth;

  if (timeOnPage < 10000 && depth < 25) return 'bounce';
  if (depth > 75 && timeOnPage > 30000) return 'engaged';
  if (depth > 50 && timeOnPage > 15000) return 'reader';
  if (depth > 30 && timeOnPage < 20000) return 'scanner';
  return 'skimmer';
}

function accumulatedTimes(state, now) {
  const elapsed = now - state.lastVisibilityChange;
  return {
    visible: Math.round(state.visibleTime + (state.isVisible ? elapsed : 0)),
    hidden: Math.round(state.hiddenTime + (state.isVisible ? 0 : elapsed))
  };
}

function sinceLoad(state, timestamp) {
  return timestamp ? Math.round(timestamp - state.pageLoadTime) : null;
}

export function buildEngagementData(state) {
  const now = Date.now();
  const times = accumulatedTimes(state, now);

  return {
    page_url: window.location.pathname,
    time_on_page_ms: Math.round(now - state.pageLoadTime),
    max_scroll_depth: state.maxScrollDepth,
    click_count: state.clickCount,
    focus_time_ms: times.visible,
    blur_count: state.tabSwitches || 0,
    tab_switches: state.tabSwitches || 0,
    visible_time_ms: times.visible,
    hidden_time_ms: times.hidden,
    time_to_first_interaction_ms: sinceLoad(state, state.firstInteractionTime),
    time_to_first_scroll_ms: sinceLoad(state, state.firstScrollTime),
    scroll_velocity_avg: calculateScrollVelocity(state),
    scroll_direction_changes: state.scrollDirectionChanges,
    mouse_move_distance_px: Math.round(state.mouseDistance),
    keyboard_events: state.keyboardEvents,
    copy_events: state.copyEvents,
    is_rage_click: state.hasRageClick,
    is_dead_click: state.hasDeadClick,
    reading_pattern: detectReadingPattern(state)
  };
}
