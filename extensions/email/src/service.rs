//! The SMTP transport.
//!
//! Ported from `systemprompt-web/extensions/email/src/service.rs`, trimmed to
//! the transport itself: the original's welcome / magic-link / daily-report
//! senders are web-signup concerns with no analogue here, and the templates
//! they rendered came with them.
//!
//! Two deliberate changes from the original:
//!
//! 1. `from_env` returns a `Result`, not an `Option`. The original logged a
//!    `warn!` per missing key and returned `None`, which reaches a caller as an
//!    undifferentiated "no email service" — useless to whoever has to fix it.
//!    Here the missing keys are collected and named in the error.
//! 2. [`EmailService::send_plain`] **mints and returns the Message-ID**. The
//!    original let lettre generate one and discarded it along with the whole
//!    `SmtpResponse`. That id is the join key between a sent mail and the Odoo
//!    chatter row that records it, so it has to be knowable before the send and
//!    survive after it.

use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::error::EmailError;

// Why: Env var first, then the active profile's `secrets.json`. Matches how
// every other secret in this repo is read (`SecretsBootstrap`), so a container
// can override a profile without editing it.
fn read_secret(env_key: &str, secrets_key: &str) -> Option<String> {
    std::env::var(env_key).ok().or_else(|| {
        systemprompt::config::SecretsBootstrap::get()
            .ok()
            .and_then(|s| s.get(secrets_key).cloned())
    })
}

/// One plain-text message, as the caller wants it sent.
///
/// A struct rather than six positional arguments: it keeps `send_plain` under
/// the argument limit, and it means adding a field later — a bcc, an
/// attachment — neither re-opens that question nor silently reorders a call
/// site.
#[derive(Debug, Clone, Copy)]
pub struct OutboundMessage<'a> {
    pub to: &'a [String],
    pub subject: &'a str,
    pub body: &'a str,
    pub reply_to: Option<&'a str>,
    // Why: the RFC5322 Message-ID minted for this send, not the platform's
    // `MessageId`. It is the join key to the Odoo chatter row and the outbox.
    pub rfc5322_id: &'a str,
}

/// An authenticated SMTP relay plus the identity we are allowed to send as.
pub struct EmailService {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

// Why: the derived Debug would print the transport's credentials.
impl std::fmt::Debug for EmailService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailService")
            .field("from", &self.from)
            .finish_non_exhaustive()
    }
}

impl EmailService {
    // Why: Builds the transport from `smtp_host` / `smtp_port` / `smtp_username` /
    // `smtp_password` / `smtp_from`.
    //
    // [`EmailError::NotConfigured`] naming every missing key, or
    // [`EmailError::BadRequest`] if `smtp_from` will not parse as a mailbox.
    pub fn from_env() -> Result<Self, EmailError> {
        let host = read_secret("SMTP_HOST", "smtp_host");
        let username = read_secret("SMTP_USERNAME", "smtp_username");
        let password = read_secret("SMTP_PASSWORD", "smtp_password");

        // Why: report every missing key at once. Reporting the first one turns
        // configuring a fresh profile into three restarts.
        let missing: Vec<&str> = [
            ("smtp_host", host.is_none()),
            ("smtp_username", username.is_none()),
            ("smtp_password", password.is_none()),
        ]
        .into_iter()
        .filter_map(|(key, absent)| absent.then_some(key))
        .collect();
        if !missing.is_empty() {
            return Err(EmailError::NotConfigured(missing.join(", ")));
        }
        let (host, username, password) = (
            host.unwrap_or_default(),
            username.unwrap_or_default(),
            password.unwrap_or_default(),
        );

        let port: u16 = read_secret("SMTP_PORT", "smtp_port")
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);

        let from_str = read_secret("SMTP_FROM", "smtp_from").unwrap_or_else(|| username.clone());
        let from = parse_from(&from_str)?;

        let credentials = Credentials::new(username, password);
        // Why: a knob and not a constant — a relay reached over the
        // public internet must use STARTTLS, and that is the default and what
        // production uses. A relay on localhost — a postfix sidecar, or the
        // capture server the e2e suite runs — has no TLS to negotiate, and
        // without an opt-out there is no way to exercise the real lettre send
        // path in a test at all. Opting in is explicit and logs a warning, so
        // it cannot be reached by a typo or an omitted key.
        let transport = match read_secret("SMTP_SECURITY", "smtp_security").as_deref() {
            Some("plaintext") => {
                tracing::warn!(
                    host,
                    port,
                    "smtp_security=plaintext: sending over an unencrypted connection. This is \
                     only correct for a relay on the local host."
                );
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
                    .port(port)
                    .credentials(credentials)
                    .build()
            },
            _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)?
                .port(port)
                .credentials(credentials)
                .build(),
        };

        Ok(Self { transport, from })
    }

    // Why: The address this deployment is permitted to send as.
    #[must_use]
    pub const fn mailbox(&self) -> &Mailbox {
        &self.from
    }

    // Why: The domain of the `From:` address, used to mint Message-IDs that match
    // the sending domain.
    #[must_use]
    pub fn sender_domain(&self) -> &str {
        self.from.email.domain()
    }

    // Why: Sends a plain-text message and returns the RFC5322 Message-ID that went
    // on the wire.
    //
    // `from` is always this deployment's own address — never the recipient's
    // or a customer's. Prod Odoo's relay is configured with
    // `from_filter = systemprompt.io` precisely so a forged `From:` cannot
    // fail SPF at the relay, and this transport keeps that same
    // discipline: an address we want replies to go to belongs in
    // `reply_to`, never in `from`.
    //
    // [`EmailError::BadRequest`] for an empty recipient list or an address
    // that will not parse; [`EmailError::Build`] / [`EmailError::Smtp`]
    // otherwise.
    pub async fn send_plain(&self, message: &OutboundMessage<'_>) -> Result<String, EmailError> {
        let OutboundMessage {
            to,
            subject,
            body,
            reply_to,
            rfc5322_id,
        } = *message;
        if to.is_empty() {
            return Err(EmailError::BadRequest(
                "at least one recipient is required".to_owned(),
            ));
        }

        let mut builder = Message::builder()
            .from(self.from.clone())
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            // Why: an explicit id rather than lettre's generated one. See the
            // module docs — this value is the join key to the Odoo chatter row.
            .message_id(Some(rfc5322_id.to_owned()));

        for address in to {
            let mailbox: Mailbox = address.parse().map_err(|e| {
                EmailError::BadRequest(format!("invalid recipient address {address:?}: {e}"))
            })?;
            builder = builder.to(mailbox);
        }

        if let Some(reply_to) = reply_to {
            let mailbox: Mailbox = reply_to.parse().map_err(|e| {
                EmailError::BadRequest(format!("invalid reply-to address {reply_to:?}: {e}"))
            })?;
            builder = builder.reply_to(mailbox);
        }

        let email = builder.body(body.to_owned())?;
        self.transport.send(email).await?;

        Ok(rfc5322_id.to_owned())
    }

    // Why: Escape hatch for a fully hand-built message.
    //
    // [`EmailError::Smtp`] if the relay refuses it.
    pub async fn send_raw(&self, message: Message) -> Result<(), EmailError> {
        self.transport.send(message).await?;
        Ok(())
    }
}

// Why: Mints an RFC5322 Message-ID in the `<uuid@domain>` form.
#[must_use]
pub fn mint_message_id(domain: &str) -> String {
    format!("<{}@{}>", uuid::Uuid::new_v4(), domain)
}

// Why: Parses `smtp_from`, which is written either bare
// (`hello@systemprompt.io`) or as a display name (`systemprompt.io
// <hello@systemprompt.io>`). The display-name form is what the production
// profile actually holds, and it is unquoted — which `Mailbox::from_str`
// rejects — so it is quoted and retried.
fn parse_from(from_str: &str) -> Result<Mailbox, EmailError> {
    let bad = |e: &dyn std::fmt::Display| {
        EmailError::BadRequest(format!(
            "smtp_from {from_str:?} is not a valid address: {e}"
        ))
    };

    let Some(bracket) = from_str.find('<') else {
        let address = from_str.trim().parse().map_err(|e| bad(&e))?;
        return Ok(Mailbox::new(Some("systemprompt.io".to_owned()), address));
    };

    if let Ok(mailbox) = from_str.parse::<Mailbox>() {
        return Ok(mailbox);
    }
    let name = from_str[..bracket].trim();
    let addr = &from_str[bracket..];
    format!("\"{name}\" {addr}").parse().map_err(|e| bad(&e))
}
