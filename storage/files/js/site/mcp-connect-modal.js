const MODAL_ID = 'mcp-connect-modal';
const TEMPLATE_ID = 'mcp-connect-template';

function fillModal(modal, name, url) {
  modal.querySelector('.mcp-modal__title').textContent = `Connect to ${name}`;
  modal.querySelector('.mcp-modal__url').textContent = url;
  modal.querySelector('.mcp-modal__copy').dataset.url = url;
  modal.querySelector('[data-slot="claude-cmd"]').textContent =
    `claude mcp add ${name} ${url} --transport http`;
  modal.querySelector('[data-slot="mcp-json"]').textContent =
    `{\n  "mcpServers": {\n    "${name}": {\n      "url": "${url}"\n    }\n  }\n}`;
}

function bindCopy(button, url) {
  button.addEventListener('click', () => {
    navigator.clipboard.writeText(url).then(
      () => {
        button.textContent = 'Copied!';
        setTimeout(() => {
          button.textContent = 'Copy';
        }, 2000);
      },
      () => {
        button.textContent = 'Copy failed';
      }
    );
  });
}

function bindDismiss(modal) {
  const close = () => {
    modal.remove();
    document.removeEventListener('keydown', onKeydown);
  };

  function onKeydown(event) {
    if (event.key === 'Escape') close();
  }

  modal.querySelector('.mcp-modal__close').addEventListener('click', close);
  modal.querySelector('.mcp-modal__backdrop').addEventListener('click', close);
  document.addEventListener('keydown', onKeydown);
}

export function showConnectModal(url, name) {
  const template = document.getElementById(TEMPLATE_ID);
  if (!(template instanceof HTMLTemplateElement)) return;

  document.getElementById(MODAL_ID)?.remove();

  const modal = template.content.firstElementChild.cloneNode(true);
  modal.id = MODAL_ID;

  fillModal(modal, name, url);
  bindCopy(modal.querySelector('.mcp-modal__copy'), url);
  bindDismiss(modal);

  document.body.append(modal);
  modal.querySelector('.mcp-modal__close').focus();
}
