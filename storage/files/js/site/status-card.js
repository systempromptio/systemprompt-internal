import { STATUS_CONFIG, extractList, fetchWithRetry } from './status-api.js';
import { renderAgents, renderPlaceholder, renderServices } from './status-render.js';
import { showConnectModal } from './mcp-connect-modal.js';

const ERROR_CLASS = ' status-card__item-indicator--error';
const WARN_CLASS = ' status-card__item-indicator--warning';

class StatusCard {
  constructor(container, mcpEndpoint, agentsEndpoint) {
    this.container = container;
    this.mcpEndpoint = mcpEndpoint;
    this.agentsEndpoint = agentsEndpoint;
    this.indicator = container.querySelector('#status-indicator');
    this.mcpList = container.querySelector('#mcp-services-list');
    this.mcpCount = container.querySelector('#mcp-count');
    this.agentsList = container.querySelector('#agents-list');
    this.agentsCount = container.querySelector('#agents-count');
    this.failureCount = 0;
    this.intervalId = null;
  }

  setIndicator(modifier) {
    if (!this.indicator) return;
    this.indicator.className = modifier
      ? `status-card__indicator status-card__indicator--${modifier}`
      : 'status-card__indicator';
  }

  schedule() {
    if (this.intervalId) return;
    this.intervalId = setInterval(() => this.refresh(), STATUS_CONFIG.refreshInterval);
  }

  unschedule() {
    if (!this.intervalId) return;
    clearInterval(this.intervalId);
    this.intervalId = null;
  }

  start() {
    this.setIndicator('loading');
    this.bindConnectDelegation();
    this.refresh();
    this.schedule();

    window.addEventListener('beforeunload', () => this.unschedule());
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.unschedule();
        return;
      }
      this.refresh();
      this.schedule();
    });
  }

  bindConnectDelegation() {
    this.container.addEventListener('click', (event) => {
      const button = event.target instanceof Element
        ? event.target.closest('.status-card__connect-btn')
        : null;
      if (!button) return;
      event.preventDefault();
      showConnectModal(button.dataset.url, button.dataset.name);
    });
  }

  async refresh() {
    try {
      const [mcp, agents] = await Promise.all([
        fetchWithRetry(this.mcpEndpoint),
        fetchWithRetry(this.agentsEndpoint)
      ]);
      this.applyServices(extractList(mcp, 'data', 'servers', 'services'));
      this.applyAgents(extractList(agents, 'data', 'agents'));
      this.setIndicator('');
      this.failureCount = 0;
    } catch {
      this.handleFailure();
    }
  }

  applyServices(services) {
    if (services.length === 0) {
      renderPlaceholder(this.mcpList, this.mcpCount, WARN_CLASS, 'No services registered');
      return;
    }
    renderServices(this.mcpList, this.mcpCount, services);
  }

  applyAgents(agents) {
    if (agents.length === 0) {
      renderPlaceholder(this.agentsList, this.agentsCount, WARN_CLASS, 'No agents active');
      return;
    }
    renderAgents(this.agentsList, this.agentsCount, agents);
  }

  handleFailure() {
    this.failureCount += 1;
    if (this.failureCount < STATUS_CONFIG.maxRetries) return;

    this.setIndicator('offline');
    renderPlaceholder(this.mcpList, this.mcpCount, ERROR_CLASS, 'Connection failed');
    renderPlaceholder(this.agentsList, this.agentsCount, ERROR_CLASS, 'Connection failed');
  }
}

export function initStatusCard() {
  const card = document.getElementById('system-status-card');
  if (!card) return;

  const { mcpEndpoint, agentsEndpoint } = card.dataset;
  if (!mcpEndpoint || !agentsEndpoint) return;

  new StatusCard(card, mcpEndpoint, agentsEndpoint).start();
}
