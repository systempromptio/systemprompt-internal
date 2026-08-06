// Renders a sign-in failure explanation when the server bounces the browser
// back to /admin/login with an `?auth=<code>` query parameter. Sign-in itself
// is passkey-based and lives in /js/services/webauthn-login.js; this file only
// surfaces the server's reason for refusing before the passkey step is reached.
const AUTH_MESSAGES = {
  not_provisioned: 'No account exists for that email yet. Ask an administrator to create your account, then sign in again.',
  seat_limit: 'Your organization has used all of its seats. Ask your administrator to free a seat or raise your plan limit.',
  forbidden: 'That email address is not permitted. Use your work email.',
  unverified: 'Your email address is not verified. Verify it, then try again.',
  expired: 'Your session expired. Sign in again to continue.',
  error: 'Sign-in failed. Please try again.'
};

const params = new URLSearchParams(window.location.search);

const authStatus = params.get('auth');
if (authStatus) {
  const errEl = document.getElementById('error');
  if (errEl) {
    errEl.textContent = AUTH_MESSAGES[authStatus] || AUTH_MESSAGES.error;
    errEl.removeAttribute('hidden');
  }
}
