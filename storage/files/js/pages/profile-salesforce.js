import { showToast } from '/js/services/toast.js';
import { rawResponse, errorMessage } from '/js/services/api.js';

const params = new URLSearchParams(window.location.search);
const outcome = params.get('sf');
if (outcome) {
  const messages = {
    linked: ['Salesforce account connected.', 'success'],
    already_linked: ['That Salesforce account is already connected to a different user.', 'error'],
    error: ['Connecting Salesforce failed. Please try again.', 'error'],
    unavailable: ['Salesforce sign-in is not configured.', 'error'],
    denied: ['Salesforce access was denied.', 'error'],
    forbidden: ['That Salesforce account’s email domain is not allowed.', 'error'],
    unverified: ['Your Salesforce email address is not verified.', 'error'],
    no_email: ['Salesforce returned no email for your account.', 'error'],
  };
  const [message, type] = messages[outcome] || messages.error;
  showToast(message, type);
  params.delete('sf');
  const query = params.toString();
  window.history.replaceState({}, '', window.location.pathname + (query ? '?' + query : ''));
}

document.getElementById('sf-unlink-btn')?.addEventListener('click', async (e) => {
  if (!window.confirm('Disconnect your Salesforce account? You will keep signing in with your passkey.')) return;
  const btn = e.currentTarget;
  btn.disabled = true;
  try {
    const resp = await rawResponse('/admin/api/profile/salesforce/unlink', { method: 'POST' });
    if (!resp.ok) throw new Error(await errorMessage(resp) || 'Disconnect failed');
    showToast('Salesforce account disconnected.', 'success');
    window.setTimeout(() => window.location.reload(), 800);
  } catch (err) {
    btn.disabled = false;
    showToast(err.message || 'Disconnect failed. Please try again.', 'error');
  }
});
