const SSO_MESSAGES = {
  unavailable: 'Salesforce sign-in is not available right now. Use a passkey, or contact the platform team.',
  denied: 'Salesforce sign-in was cancelled.',
  forbidden: 'That Salesforce account is not permitted. Use your Astound work email.',
  unverified: 'Your Salesforce email is not verified. Verify it in Salesforce, then try again.',
  no_email: 'Salesforce did not return an email address for your account.',
  not_provisioned: 'No account exists for that Salesforce identity yet. Ask an administrator to create your account, then sign in again.',
  seat_limit: 'Your organization has used all of its seats. Ask your administrator to free a seat or raise your plan limit.',
  error: 'Salesforce sign-in failed. Please try again.'
};

const params = new URLSearchParams(window.location.search);

const sso = document.getElementById('salesforce-login');
const redirect = params.get('redirect');
if (sso && redirect) {
  sso.href = `/admin/auth/salesforce/start?redirect=${encodeURIComponent(redirect)}`;
}

const ssoStatus = params.get('sso');
if (ssoStatus) {
  const errEl = document.getElementById('error');
  if (errEl) {
    errEl.textContent = SSO_MESSAGES[ssoStatus] || SSO_MESSAGES.error;
    errEl.removeAttribute('hidden');
  }
}
