import { rawFetch } from '../services/api.js';

const RESEND_LABEL = 'Resend Verification Email';
const COOLDOWN_SECONDS = 60;
const TICK_MS = 1000;

let cooldown = false;

const startCooldown = (btn) => {
  cooldown = true;
  let seconds = COOLDOWN_SECONDS;
  btn.textContent = `Resend in ${seconds}s`;
  const interval = setInterval(() => {
    seconds -= 1;
    if (seconds <= 0) {
      clearInterval(interval);
      cooldown = false;
      btn.disabled = false;
      btn.textContent = RESEND_LABEL;
    } else {
      btn.textContent = `Resend in ${seconds}s`;
    }
  }, TICK_MS);
};

const showResendError = (message) => {
  const errorEl = document.getElementById('resend-error');
  errorEl.textContent = message;
  errorEl.hidden = false;
  document.getElementById('resend-success').hidden = true;
};

const sendVerification = async (btn, email) => {
  btn.disabled = true;
  btn.textContent = 'Sending...';
  document.getElementById('resend-error').hidden = true;
  document.getElementById('resend-success').hidden = true;
  try {
    await rawFetch('/api/public/auth/resend-verification', {
      method: 'POST',
      body: JSON.stringify({ email }),
    });
    document.getElementById('resend-success').hidden = false;
    startCooldown(btn);
  } catch (error) {
    showResendError(error.message || 'Something went wrong. Please try again.');
    btn.disabled = false;
    btn.textContent = RESEND_LABEL;
  }
};

const handleResend = async (event) => {
  const btn = event.currentTarget;
  const email = document.getElementById('resend-email').value.trim();
  if (!cooldown) {
    if (email.includes('@')) {
      await sendVerification(btn, email);
    } else {
      showResendError('Please enter a valid email address.');
    }
  }
};

const init = () => {
  document.getElementById('resend-btn').addEventListener('click', handleResend);
};

init();
