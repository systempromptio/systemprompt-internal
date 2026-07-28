const ICONS = { success: '✓', error: '✗', info: 'ⓘ', warning: '⚠' };
const TYPES = ['success', 'error', 'info', 'warning'];
const DISMISS_MS = 4000;
const LEAVE_MS = 350;

const sheet = new CSSStyleSheet();
sheet.replaceSync(`
  :host {
    position: fixed;
    top: var(--sp-space-4);
    right: var(--sp-space-4);
    z-index: var(--sp-z-toast);
    display: flex;
    flex-direction: column;
    gap: var(--sp-space-2);
    max-width: 360px;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: var(--sp-space-2);
    padding: var(--sp-space-3) var(--sp-space-4);
    background: var(--sp-bg-surface-overlay);
    border: 1px solid var(--sp-border-default);
    border-left: 3px solid var(--sp-info);
    border-radius: var(--sp-radius-md);
    box-shadow: var(--sp-shadow-float);
    color: var(--sp-text-primary);
    font-size: var(--sp-text-sm);
    animation: toast-in var(--sp-duration-slow) var(--sp-ease-out) forwards;
  }
  .toast.is-leaving { opacity: 0; transition: opacity ${LEAVE_MS}ms var(--sp-ease-out); }
  .toast[data-type="success"] { border-left-color: var(--sp-success); }
  .toast[data-type="error"] { border-left-color: var(--sp-danger); }
  .toast[data-type="warning"] { border-left-color: var(--sp-warning); }
  .icon { flex-shrink: 0; }
  .toast[data-type="success"] .icon { color: var(--sp-success); }
  .toast[data-type="error"] .icon { color: var(--sp-danger); }
  .toast[data-type="warning"] .icon { color: var(--sp-warning); }
  .toast[data-type="info"] .icon { color: var(--sp-info); }
  @keyframes toast-in {
    from { opacity: 0; transform: translateX(24px); }
    to { opacity: 1; transform: translateX(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    .toast { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
  }
  @media (max-width: 640px) {
    :host { left: var(--sp-space-3); right: var(--sp-space-3); max-width: none; }
  }
`);

const template = document.createElement('template');
template.innerHTML = `
  <div class="toast" role="status">
    <span class="icon" aria-hidden="true"></span>
    <span class="message"></span>
  </div>
`;

export class SpToast extends HTMLElement {
  #timeouts = new Set();

  connectedCallback() {
    if (!this.shadowRoot) {
      this.attachShadow({ mode: 'open' });
      this.shadowRoot.adoptedStyleSheets = [sheet];
      this.setAttribute('aria-live', 'polite');
    }
  }

  disconnectedCallback() {
    for (const id of this.#timeouts) {
      clearTimeout(id);
    }
    this.#timeouts.clear();
  }

  show(message, type = 'info') {
    this.connectedCallback();
    const kind = TYPES.includes(type) ? type : 'info';
    const fragment = template.content.cloneNode(true);
    const el = fragment.querySelector('.toast');
    el.dataset.type = kind;
    fragment.querySelector('.icon').textContent = ICONS[kind];
    fragment.querySelector('.message').textContent = message;
    this.shadowRoot.append(fragment);
    const dismiss = setTimeout(() => {
      el.classList.add('is-leaving');
      const remove = setTimeout(() => el.remove(), LEAVE_MS);
      this.#timeouts.add(remove);
    }, DISMISS_MS);
    this.#timeouts.add(dismiss);
  }
}

if (!customElements.get('sp-toast')) {
  customElements.define('sp-toast', SpToast);
}
