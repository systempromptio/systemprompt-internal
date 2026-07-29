const byId = (id) => document.getElementById(id);

export const showError = (msg) => {
  const errorEl = byId('error');
  errorEl.textContent = msg;
  errorEl.hidden = false;
  byId('loading').hidden = true;
  byId('register-form').hidden = false;
  byId('retry').hidden = false;
};

export const showLoading = (msg) => {
  byId('loading-text').textContent = msg;
  byId('register-form').hidden = true;
  byId('error').hidden = true;
  byId('loading').hidden = false;
};

export const showUnsupported = (msg) => {
  const errorEl = byId('error');
  errorEl.textContent = msg;
  errorEl.hidden = false;
  byId('register-form').querySelector('button[type="submit"]').disabled = true;
};

export const describeError = (error) => {
  if (error.name === 'NotAllowedError') {
    return 'Passkey creation was cancelled or not allowed.';
  }
  if (error.name === 'NotSupportedError') {
    return 'Passkeys are not supported on this device.';
  }
  return error.message || 'Registration failed. Please try again.';
};
