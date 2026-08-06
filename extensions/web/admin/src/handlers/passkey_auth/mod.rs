//! Email + passkey self-registration.
//!
//! `POST /admin/auth/passkey/register` is the domain-gated front door for
//! creating an account without an operator in the loop: it validates the email against the
//! same `allowed_email_domains` list that gates SSO, provisions the user (org
//! membership and seat limit included), and returns a short-lived setup token.
//! The browser then enrols the passkey through core's public
//! `/api/v1/core/oauth/webauthn/link/{start,finish}` ceremony — the same flow
//! CLI-created users use — and signs in with the fresh credential.
//!
//! Core's own open `/webauthn/register/*` endpoints are disabled via
//! `security.allow_registration: false`; this endpoint is the only
//! self-registration door and the allowlist is its whole gate.

mod register;

pub(crate) use register::passkey_register;
