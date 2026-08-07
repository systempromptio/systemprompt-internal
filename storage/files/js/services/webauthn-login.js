import {
  getEmailInput, clearAccessToken, showError, showLoginForm,
  showLoading, showPasskeyError, showEmailError, initPaneToggles
} from '/js/services/webauthn-login-ui.js?v=4';
import { hasValidAdminToken } from '/js/services/admin-token.js';
import { startPasskeyAuth, finishPasskeyAuth, redirectWithPkce } from '/js/services/webauthn-helpers.js?v=3';
import {
  DEFAULT_REDIRECT, resolveRedirect, exchangeToken,
  storeSession, completePendingRegistration
} from '/js/services/webauthn-session.js';
import { safeStorageGet, safeStorageRemove } from '/js/utils/storage-safe.js';

const LOGIN_PATH = '/admin/login/operator';
const signInBtn = document.getElementById('sign-in-btn');
const emailInput = getEmailInput();
let isAuthenticating = false;

const processCallback = async (code) => {
  const codeVerifier = safeStorageGet('pkce_code_verifier');
  const redirectAfterLogin = safeStorageGet('login_redirect') || DEFAULT_REDIRECT;
  if (!codeVerifier) {
    window.history.replaceState({}, '', LOGIN_PATH);
    showLoginForm();
    return;
  }
  try {
    showLoading('Exchanging token...');
    const tokenData = await exchangeToken(code, codeVerifier, window.location.origin + LOGIN_PATH);
    await storeSession(tokenData);
    await completePendingRegistration();
    safeStorageRemove('pkce_code_verifier');
    safeStorageRemove('pkce_csrf_state');
    safeStorageRemove('login_redirect');
    window.location.href = await resolveRedirect(redirectAfterLogin);
  } catch (err) {
    showError(err.message);
    window.history.replaceState({}, '', LOGIN_PATH);
  }
};

const handleCallback = async () => {
  const params = new URLSearchParams(window.location.search);
  const error = params.get('error');
  if (error) {
    showError(params.get('error_description') || error);
    return true;
  }
  const code = params.get('code');
  if (code) {
    await processCallback(code);
    return true;
  }
  return false;
};

const authenticateWithPasskey = async () => {
  if (isAuthenticating) return;
  const email = emailInput.value.trim();
  if (!email) {
    showEmailError('Please enter your email address.');
    return;
  }
  isAuthenticating = true;
  signInBtn.disabled = true;
  try {
    const { startResponse, credential } = await startPasskeyAuth(email);
    const finishResponse = await finishPasskeyAuth(startResponse, credential);
    await redirectWithPkce(finishResponse);
  } catch (error) {
    showPasskeyError(error);
  } finally {
    isAuthenticating = false;
    signInBtn.disabled = false;
  }
};

const startLoginPage = async () => {
  if (await handleCallback()) return;
  if (hasValidAdminToken()) {
    const params = new URLSearchParams(window.location.search);
    window.location.href = await resolveRedirect(params.get('redirect'));
    return;
  }
  await clearAccessToken();
  if (window.PublicKeyCredential) {
    showLoginForm();
  } else {
    showError('Your browser does not support passkeys. Please use a modern browser (Chrome, Firefox, Safari, Edge).');
  }
};

signInBtn.addEventListener('click', authenticateWithPasskey);
emailInput.addEventListener('keypress', (e) => {
  if (e.key === 'Enter') { e.preventDefault(); authenticateWithPasskey(); }
});
initPaneToggles();
startLoginPage();
