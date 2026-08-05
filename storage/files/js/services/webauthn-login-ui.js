import { showToast } from '/js/services/toast.js';
import { rawResponse } from '/js/services/api.js';

const errorDiv = document.getElementById('error');
const loadingSection = document.getElementById('loading');
const loadingText = document.getElementById('loading-text');
const retrySection = document.getElementById('retry');
const loginForm = document.getElementById('login-form');
const emailInput = document.getElementById('login-email');
const registerForm = document.getElementById('register-form');

export const getEmailInput = () => emailInput;

export async function clearAccessToken() {
  try {
    await rawResponse('/api/public/auth/session', { method: 'DELETE' });
  } catch {
    showToast('Failed to clear session. Please try again.', 'error');
  }
  document.cookie = 'access_token=; path=/; max-age=0; SameSite=Lax' +
    (window.location.protocol === 'https:' ? '; Secure' : '');
}

export async function showError(msg) {
  await clearAccessToken();
  errorDiv.textContent = msg;
  errorDiv.hidden = false;
  loadingSection.hidden = true;
  loginForm.hidden = true;
  if (registerForm) registerForm.hidden = true;
  retrySection.hidden = false;
}

export function showLoginForm() {
  loginForm.hidden = false;
  loadingSection.hidden = true;
  retrySection.hidden = true;
  errorDiv.hidden = true;
}

export function showLoading(msg) {
  loadingText.textContent = msg || 'Processing...';
  loginForm.hidden = true;
  if (registerForm) registerForm.hidden = true;
  loadingSection.hidden = false;
  retrySection.hidden = true;
}

function setErrorMessage(msg, correctUrl) {
  errorDiv.textContent = msg;
  if (correctUrl) {
    errorDiv.append(' ');
    const link = document.createElement('a');
    link.href = correctUrl;
    link.textContent = 'Continue on ' + new URL(correctUrl).hostname;
    errorDiv.append(link);
  }
  errorDiv.hidden = false;
}

export function showPasskeyError(error) {
  loadingSection.hidden = true;
  loginForm.hidden = false;
  if (error.name === 'RpIdMismatchError') setErrorMessage(error.message, error.correctUrl);
  else if (error.name === 'NotAllowedError') setErrorMessage('Authentication was cancelled or not allowed.');
  else if (error.name === 'NotSupportedError') setErrorMessage('Passkeys are not supported on this device.');
  else if (error.name === 'SecurityError') {
    setErrorMessage(
      'This page\'s address does not match the domain these passkeys are registered to, ' +
      'so the browser refused the sign in.',
      error.correctUrl
    );
  } else setErrorMessage(error.message || 'Authentication failed. Please try again.', error.correctUrl);
}

export function showEmailError(msg) {
  errorDiv.textContent = msg;
  errorDiv.hidden = false;
}

export function showRegisterError(msg) {
  loadingSection.hidden = true;
  if (registerForm) registerForm.hidden = false;
  setErrorMessage(msg);
}

export function initPaneToggles() {
  const ssoBlock = document.querySelector('.sso-block');
  const showPane = (showRegister) => (e) => {
    e.preventDefault();
    errorDiv.hidden = true;
    if (registerForm) registerForm.hidden = !showRegister;
    loginForm.hidden = showRegister;
    if (ssoBlock) ssoBlock.hidden = showRegister;
  };
  document.getElementById('show-register')?.addEventListener('click', showPane(true));
  document.getElementById('show-signin')?.addEventListener('click', showPane(false));
}
