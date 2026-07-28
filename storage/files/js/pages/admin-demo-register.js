import { apiFetch } from '../services/api.js';
import { showToast } from '../services/toast.js';

const SUBMIT_LABEL = 'Create User & Generate Registration Link';
const COPY_RESET_MS = 2000;

let copyTimeout;

const showDemoError = (message) => {
  const errorEl = document.getElementById('demo-error');
  errorEl.textContent = message;
  errorEl.hidden = false;
};

const createDemoUser = async () => {
  const name = document.getElementById('demo-name').value.trim();
  const email = document.getElementById('demo-email').value.trim();
  const role = document.getElementById('demo-role').value;
  const result = await apiFetch('/demo-register', {
    method: 'POST',
    body: JSON.stringify({ name, email, role }),
  });
  if (result?.ok) {
    document.getElementById('demo-link').value =
      window.location.origin + result.registration_url;
    document.getElementById('demo-result').hidden = false;
    document.getElementById('demo-register-form').reset();
  } else {
    showDemoError(result?.error || 'Failed to create user');
  }
};

const handleSubmit = async (event) => {
  event.preventDefault();
  const submitBtn = document.getElementById('demo-submit-btn');
  document.getElementById('demo-error').hidden = true;
  document.getElementById('demo-result').hidden = true;
  submitBtn.disabled = true;
  submitBtn.textContent = 'Creating...';
  try {
    await createDemoUser();
  } catch (error) {
    showDemoError(error.message || 'Failed to create user');
  } finally {
    submitBtn.disabled = false;
    submitBtn.textContent = SUBMIT_LABEL;
  }
};

const handleCopy = async (event) => {
  const btn = event.currentTarget;
  const link = document.getElementById('demo-link');
  link.select();
  try {
    await navigator.clipboard.writeText(link.value);
    btn.textContent = 'Copied!';
    clearTimeout(copyTimeout);
    copyTimeout = setTimeout(() => {
      btn.textContent = 'Copy Link';
    }, COPY_RESET_MS);
  } catch (error) {
    showToast(error.message || 'Could not copy the link. Copy it manually.', 'error');
  }
};

const init = () => {
  document.getElementById('demo-register-form').addEventListener('submit', handleSubmit);
  document.getElementById('demo-copy-btn').addEventListener('click', handleCopy);
};

init();
