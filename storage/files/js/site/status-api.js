export const STATUS_CONFIG = {
  refreshInterval: 30000,
  retryDelay: 5000,
  maxRetries: 3
};

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function fetchJson(endpoint) {
  const response = await fetch(endpoint, {
    method: 'GET',
    headers: { Accept: 'application/json' }
  });

  if (!response.ok) {
    throw new Error(`${endpoint} responded ${response.status}`);
  }

  return response.json();
}

export async function fetchWithRetry(endpoint, retries = STATUS_CONFIG.maxRetries) {
  let lastError = new Error(`${endpoint} was never attempted`);

  for (let attempt = 0; attempt < retries; attempt += 1) {
    try {
      return await fetchJson(endpoint);
    } catch (error) {
      lastError = error;
      if (attempt < retries - 1) {
        await delay(STATUS_CONFIG.retryDelay);
      }
    }
  }

  throw lastError;
}

export function extractList(data, ...keys) {
  if (Array.isArray(data)) return data;
  if (!data || typeof data !== 'object') return [];

  for (const key of keys) {
    if (Array.isArray(data[key])) return data[key];
  }

  return [];
}

export function statusClassFor(item) {
  const status = String(item.status || item.state || 'running').toLowerCase();
  if (status === 'error' || status === 'failed') return ' status-card__item-indicator--error';
  if (status === 'warning' || status === 'degraded') return ' status-card__item-indicator--warning';
  return '';
}

export function scopesFor(item) {
  const direct = item.oauth_scopes || item.scopes;
  if (Array.isArray(direct) && direct.length) return direct;

  const oauth2 = item.security?.[0]?.oauth2;
  return Array.isArray(oauth2) ? oauth2 : [];
}
