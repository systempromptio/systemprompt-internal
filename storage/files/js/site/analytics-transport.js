import { ENDPOINT, MIN_TIME_MS } from './analytics-state.js';
import { buildEngagementData, calculateScrollVelocity } from './analytics-metrics.js';

function ignoreTransportFailure() {
  return undefined;
}

function postKeepalive(body) {
  if (typeof fetch !== 'function') return;
  fetch(ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'same-origin',
    keepalive: true,
    body
  }).then(ignoreTransportFailure, ignoreTransportFailure);
}

export function sendEvent(eventType, eventData) {
  const body = JSON.stringify({
    page_url: window.location.pathname,
    data: { ...eventData, event_type: eventType }
  });

  if (typeof navigator.sendBeacon === 'function') {
    const queued = navigator.sendBeacon(ENDPOINT, new Blob([body], { type: 'application/json' }));
    if (queued) return;
  }

  postKeepalive(body);
}

export function sendPageView(state) {
  if (state.pageViewSent) return;
  state.pageViewSent = true;

  sendEvent('page_view', {
    referrer: document.referrer || null,
    title: document.title
  });
}

export function sendPageExit(state) {
  if (state.dataSent) return;
  if (Date.now() - state.pageLoadTime < MIN_TIME_MS) return;

  state.dataSent = true;
  sendEvent('page_exit', buildEngagementData(state));
}

export function sendScrollMilestone(state, milestone) {
  if (state.scrollMilestonesSent[milestone]) return;
  state.scrollMilestonesSent[milestone] = true;

  sendEvent('scroll', {
    depth: state.maxScrollDepth,
    milestone,
    direction: state.lastScrollDirection || 'down',
    velocity: calculateScrollVelocity(state)
  });
}

export function sendLinkClick(targetUrl, linkText, isExternal) {
  sendEvent('link_click', {
    target_url: targetUrl,
    link_text: linkText ? linkText.substring(0, 100) : null,
    is_external: isExternal
  });
}
