export const ENDPOINT = '/track/engagement';
export const MIN_TIME_MS = 5000;
export const RAGE_CLICK_THRESHOLD = 3;
export const RAGE_CLICK_WINDOW_MS = 500;
export const SCROLL_MILESTONES = [25, 50, 75, 90, 100];

export function createState() {
  return {
    pageLoadTime: Date.now(),
    firstInteractionTime: null,
    firstScrollTime: null,
    maxScrollDepth: 0,
    scrollPositions: [],
    scrollDirectionChanges: 0,
    lastScrollDirection: null,
    clickCount: 0,
    clickTimestamps: [],
    hasRageClick: false,
    hasDeadClick: false,
    mouseDistance: 0,
    lastMousePosition: null,
    keyboardEvents: 0,
    copyEvents: 0,
    focusTime: 0,
    blurCount: 0,
    tabSwitches: 0,
    visibleTime: 0,
    hiddenTime: 0,
    lastVisibilityChange: Date.now(),
    isVisible: !document.hidden,
    dataSent: false,
    pageViewSent: false,
    scrollMilestonesSent: {},
    linkClicks: []
  };
}
