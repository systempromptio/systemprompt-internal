import { scopesFor, statusClassFor } from './status-api.js';

export function createStatusItem(indicatorClass, text) {
  const li = document.createElement('li');
  li.className = 'status-card__item';

  const indicator = document.createElement('span');
  indicator.className = `status-card__item-indicator${indicatorClass}`;

  const name = document.createElement('span');
  name.className = 'status-card__item-name';
  name.textContent = text;

  li.append(indicator, name);
  return li;
}

function createEntryItem(entry) {
  const li = createStatusItem(statusClassFor(entry), entry.name || entry.id || 'Unknown');
  const scopes = scopesFor(entry);

  if (scopes.length) {
    const scopeSpan = document.createElement('span');
    scopeSpan.className = 'status-card__item-scope';
    scopeSpan.textContent = ` [${scopes.join(', ')}]`;
    li.querySelector('.status-card__item-name').append(scopeSpan);
  }

  return li;
}

function createConnectButton(name, url) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'status-card__connect-btn';
  button.dataset.url = url;
  button.dataset.name = name;
  button.title = 'Copy MCP connection URL';
  button.textContent = '[connect]';
  return button;
}

export function renderPlaceholder(list, countEl, indicatorClass, text) {
  if (!list) return;
  list.replaceChildren(createStatusItem(indicatorClass, text));
  if (countEl) countEl.textContent = '0';
}

export function renderServices(list, countEl, services) {
  if (!list) return;

  const fragment = document.createDocumentFragment();
  const baseUrl = window.location.origin;

  for (const service of services) {
    const li = createEntryItem(service);
    const endpoint = service.endpoint || '';
    if (endpoint) {
      li.append(createConnectButton(service.name || service.id || 'Unknown', baseUrl + endpoint));
    }
    fragment.append(li);
  }

  list.replaceChildren(fragment);
  if (countEl) countEl.textContent = String(services.length);
}

export function renderAgents(list, countEl, agents) {
  if (!list) return;

  const fragment = document.createDocumentFragment();
  for (const agent of agents) {
    fragment.append(createEntryItem(agent));
  }

  list.replaceChildren(fragment);
  if (countEl) countEl.textContent = String(agents.length);
}
