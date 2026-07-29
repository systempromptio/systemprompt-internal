import { showToast } from './toast.js';

export const BASE = '/admin';
export const API_BASE = '/api/public/admin';

export const rawResponse = (url, options = {}) => fetch(url, {
  headers: { 'Content-Type': 'application/json' },
  ...options
});

export const errorMessage = async (resp) => {
  const text = await resp.text();
  try {
    const json = JSON.parse(text);
    return json.error || json.message || text || resp.statusText;
  } catch {
    return text || resp.statusText;
  }
};

const request = async (url, options = {}) => {
  const resp = await rawResponse(url, options);
  if (!resp.ok) {
    const message = await errorMessage(resp);
    showToast(message, 'error');
    throw new Error(message);
  }
  const ct = resp.headers.get('content-type') || '';
  if (resp.status === 204 || !ct.includes('application/json')) return null;
  return resp.json();
};

export const apiFetch = (path, options = {}) => request(API_BASE + path, options);

export const apiGet = (path) => apiFetch(path);

export const rawFetch = (url, options = {}) => request(url, options);
