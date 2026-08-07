import { rawResponse, errorMessage } from '/js/services/api.js';
import { showToast } from '/js/services/toast.js';
import { safeStorageGet, safeStorageSet, safeStorageRemove } from '/js/utils/storage-safe.js';

const CLIENT_ID = 'marketplace-admin';
const OAUTH_BASE = '/api/v1/core/oauth';
const LOGIN_PATH = '/admin/login';

export const DEFAULT_REDIRECT = '/admin/profile';

// Mirrors the server-side `sanitize_login_redirect`: any same-origin absolute
// path is a valid post-login target (e.g. /bridge-auth/device-link), while
// protocol-relative `//host` and absolute URLs are open-redirect vectors and
// fall back to the default.
export const resolveRedirect = async (target) => {
  if (!target || !target.startsWith('/') || target.startsWith('//')) return DEFAULT_REDIRECT;
  try {
    const probe = await rawResponse(target, { method: 'HEAD' });
    if (probe.status === 404) return DEFAULT_REDIRECT;
  } catch {
    return target;
  }
  return target;
};

// `redirectUri` must be byte-identical to the one the code was issued against,
// or the token endpoint rejects the exchange. Odoo sign-in issues against
// /admin/login and the passkey ceremony against /admin/login/operator, so the
// caller passes its own rather than sharing one constant.
export const exchangeToken = async (code, codeVerifier, redirectUri) => {
  const tokenResponse = await rawResponse(OAUTH_BASE + '/token', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    credentials: 'same-origin',
    body: new URLSearchParams({
      grant_type: 'authorization_code', code,
      redirect_uri: redirectUri || window.location.origin + LOGIN_PATH,
      code_verifier: codeVerifier, client_id: CLIENT_ID,
    }),
  });
  if (!tokenResponse.ok) {
    throw new Error(await errorMessage(tokenResponse) || 'Token exchange failed');
  }
  return tokenResponse.json();
};

export const storeSession = async (tokenData) => {
  const response = await rawResponse('/api/public/auth/session', {
    method: 'POST',
    credentials: 'same-origin',
    body: JSON.stringify({
      access_token: tokenData.access_token,
      expires_in: tokenData.expires_in || 3600
    }),
  });
  if (!response.ok) {
    throw new Error(await errorMessage(response) || 'Failed to store session');
  }
  if (tokenData.refresh_token) safeStorageSet('refresh_token', tokenData.refresh_token);
};

export const completePendingRegistration = async () => {
  const pendingReg = safeStorageGet('pending_registration');
  if (!pendingReg) return;
  try {
    const response = await rawResponse('/api/public/auth/register', {
      method: 'POST',
      credentials: 'same-origin',
      body: pendingReg,
    });
    if (!response.ok) {
      showToast(await errorMessage(response) || 'Registration could not be completed.', 'error');
    }
  } catch {
    showToast('Registration could not be completed. Please try again.', 'error');
  }
  safeStorageRemove('pending_registration');
};
