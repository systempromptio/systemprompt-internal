// The sign-in page's single-sign-on outcome: an SSO round trip that fails
// comes back to /admin/login?sso=<code>, and this names the reason.
const SSO_MESSAGES = {
  unavailable: 'Single sign-on is not configured on this instance yet. Contact the platform team.',
  denied: 'Sign-in was cancelled or refused by your identity provider.',
  no_subject: 'Your identity provider did not identify your account (no NameID in the assertion). Your IT team needs to add a Name ID claim to the relying party.',
  invalid_assertion: 'Your identity provider returned a response this platform could not verify. Try again; if it persists, contact the platform team.',
  no_email: 'Your identity provider did not return an email address for your account. Contact the platform team.',
  forbidden: 'Your account is not on an email domain this platform accepts.',
  no_group: 'Your account is not in a directory group this platform maps, so it has no access here. Ask your IT team to add you to the right group.',
  not_provisioned: 'No account exists for you yet. Ask your IT team to add you to a mapped directory group.',
  no_project: 'Your account is not in a project directory group. Ask your IT team to add you to one.',
  ambiguous_project: 'Your account is in more than one project directory group. Ask your IT team to leave you in exactly one.',
  error: 'Sign-in failed. Please try again.'
};

const params = new URLSearchParams(window.location.search);

const ssoStatus = params.get('sso');
if (ssoStatus) {
  const errEl = document.getElementById('error');
  if (errEl) {
    errEl.textContent = SSO_MESSAGES[ssoStatus] || SSO_MESSAGES.error;
    errEl.hidden = false;
  }
  const retryEl = document.getElementById('retry');
  if (retryEl) {
    retryEl.hidden = false;
  }
}
