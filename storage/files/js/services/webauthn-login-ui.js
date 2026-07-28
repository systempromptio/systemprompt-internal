import { showToast } from '/js/services/toast.js';
import { rawResponse, errorMessage } from '/js/services/api.js';

const errorDiv = document.getElementById('error');
const loadingSection = document.getElementById('loading');
const loadingText = document.getElementById('loading-text');
const retrySection = document.getElementById('retry');
const loginForm = document.getElementById('login-form');
const emailInput = document.getElementById('login-email');
const magicLinkForm = document.getElementById('magic-link-form');
const magicLinkSent = document.getElementById('magic-link-sent');
const magicEmailInput = document.getElementById('magic-email');

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
  loadingSection.hidden = false;
  retrySection.hidden = true;
  if (magicLinkForm) magicLinkForm.hidden = true;
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

function showMagicLinkForm(e) {
  e.preventDefault();
  loginForm.hidden = true;
  magicLinkForm.hidden = false;
  errorDiv.hidden = true;
  if (emailInput.value.trim()) magicEmailInput.value = emailInput.value.trim();
  magicEmailInput.focus();
}

function showPasskeyFormAgain(e) {
  e.preventDefault();
  magicLinkForm.hidden = true;
  loginForm.hidden = false;
  errorDiv.hidden = true;
}

async function sendMagicLink() {
  const email = magicEmailInput.value.trim();
  if (!email) {
    showEmailError('Please enter your email address.');
    return;
  }
  try {
    errorDiv.hidden = true;
    showLoading('Sending magic link...');
    const response = await rawResponse('/api/public/auth/magic-link', {
      method: 'POST',
      body: JSON.stringify({ email }),
    });
    if (!response.ok) {
      throw new Error(await errorMessage(response) || 'Failed to send magic link');
    }
    loadingSection.hidden = true;
    magicLinkForm.hidden = true;
    magicLinkSent.hidden = false;
  } catch (err) {
    loadingSection.hidden = true;
    magicLinkForm.hidden = false;
    showToast(err.message || 'Something went wrong. Please try again.', 'error');
  }
}

export function initMagicLinkUI() {
  document.getElementById('magic-link-trigger')?.addEventListener('click', showMagicLinkForm);
  document.getElementById('back-to-passkey')?.addEventListener('click', showPasskeyFormAgain);
  document.getElementById('send-magic-btn')?.addEventListener('click', sendMagicLink);
  magicEmailInput?.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      sendMagicLink();
    }
  });
}
