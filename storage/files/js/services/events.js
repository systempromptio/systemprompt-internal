const handlers = {
  click: [],
  change: [],
  keydown: [],
  input: []
};

const outsideHandlers = [];
const escapeHandlers = [];
const keyHandlers = [];

export const on = (eventType, selector, handler, options) => {
  const entry = {
    selector,
    handler,
    exclusive: options?.exclusive || false
  };
  if (handlers[eventType]) {
    handlers[eventType].push(entry);
  }
};

export const onOutsideClick = (handler) => {
  outsideHandlers.push(handler);
};

export const onEscape = (handler) => {
  escapeHandlers.push(handler);
};

export const onKey = (key, handler) => {
  keyHandlers.push({ key, handler });
};

const dispatch = (entries, e) => {
  for (const entry of entries) {
    const match = e.target.closest(entry.selector);
    if (match) {
      entry.handler(e, match);
      if (entry.exclusive) return true;
    }
  }
  return false;
};

let closeMenusFn = null;

export const setCloseMenus = (fn) => {
  closeMenusFn = fn;
};

const onDocumentKeydown = (e) => {
  if (e.key === 'Escape') {
    if (closeMenusFn) closeMenusFn();
    for (const handler of escapeHandlers) handler(e);
  }
  for (const entry of keyHandlers) {
    if (entry.key === e.key) entry.handler(e);
  }
  dispatch(handlers.keydown, e);
};

let delegationReady = false;

export const initDelegation = () => {
  if (delegationReady) return;
  delegationReady = true;
  document.addEventListener('click', (e) => {
    for (const handler of outsideHandlers) handler(e);
  }, true);
  document.addEventListener('click', (e) => {
    const handled = dispatch(handlers.click, e);
    if (!handled && closeMenusFn) closeMenusFn();
  });
  document.addEventListener('change', (e) => dispatch(handlers.change, e));
  document.addEventListener('input', (e) => dispatch(handlers.input, e));
  document.addEventListener('keydown', onDocumentKeydown);
};
