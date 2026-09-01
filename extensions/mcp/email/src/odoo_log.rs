//! Logging an already-sent email onto its Odoo record.
//!
//! # Why this is not `note_add`, and not `message_post`'s defaults
//!
//! Odoo's `message_post` is both "write a chatter row" and "notify people"
//! depending on its kwargs, and the difference is not in `message_type`:
//!
//! * `message_type` is provenance only — `comment` means a human wrote it in
//!   the chatter, `email` means it came from or went out as mail. It does not
//!   decide whether anything is delivered.
//! * `subtype_xmlid` decides **who is notified**. `mail.mt_comment` fans out to
//!   the record's followers; `mail.mt_note` is an internal log and fans out to
//!   nobody.
//! * `partner_ids` are explicit recipients. **Any partner listed here gets a
//!   `mail.mail` created for them and is emailed.**
//!
//! We have already delivered this message over SMTP. So the combination that
//! records without re-sending is `subtype_xmlid = mail.mt_note` **and**
//! `partner_ids = []`. Passing the customer's partner id here — the intuitive
//! thing to do, since they are the recipient — is precisely what double-sends.
//!
//! The existing `OdooClient::message_post` wrapper hardcodes
//! `message_type: "comment"` and passes no `partner_ids`, so it happens to be
//! safe; that is an accident of it only ever having been used for notes, not a
//! property anyone designed. This module does not reuse it, because it needs
//! kwargs that wrapper cannot express.
//!
//! # Why the Message-ID is written back
//!
//! We mint the RFC5322 Message-ID ourselves and store the same value on
//! `mail.message.message_id`. Odoo has no inbound mail transport configured
//! today, so this buys nothing immediately. The day one is added, Odoo's
//! `message_process` matches an inbound `In-Reply-To` against exactly that
//! column, and replies thread onto the record retroactively with no migration.

use serde::Serialize;
use systemprompt_mcp_odoo::client::{Credentials, ModelCall, OdooClient};
use systemprompt_mcp_odoo::error::OdooError;

// Why: The kwargs of the `message_post` call that records a sent email.
//
// A struct rather than a `json!` literal (repository rule 8) specifically so
// `partner_ids` cannot be quietly omitted or filled in: it is a required field
// whose value is load-bearing, and the type makes that visible at every call
// site.
#[derive(Debug, Serialize)]
struct LogSentMail<'a> {
    body: &'a str,
    subject: &'a str,
    // Why: Provenance: this went out as mail.
    message_type: &'static str,
    // Why: `mail.mt_note` — an internal log that notifies no follower.
    subtype_xmlid: &'static str,
    // Why: **Always empty.** See the module docs: a non-empty list re-sends.
    partner_ids: [i64; 0],
    email_from: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<&'a str>,
    // Why: The exact Message-ID that went on the wire.
    #[serde(rename = "message_id")]
    rfc5322_id: &'a str,
}

/// What to record, and where.
#[derive(Debug)]
pub struct SentMail<'a> {
    pub res_model: &'a str,
    pub res_id: i64,
    pub subject: &'a str,
    pub body: &'a str,
    pub email_from: &'a str,
    pub reply_to: Option<&'a str>,
    pub rfc5322_id: &'a str,
}

// Why: Writes the chatter row and returns Odoo's `mail.message` id.
//
// Runs as the calling user's own Odoo credential, so Odoo's record rules
// decide whether they may post to this record and Odoo's audit log names the
// real person — the same invariant the Odoo MCP server holds to.
//
// Any Odoo transport or permission failure. The caller must treat this as
// non-fatal: the email has already been delivered by this point, so failing
// the whole tool call would misreport a successful send as a failure.
pub async fn log_sent_mail(
    client: &OdooClient,
    creds: &Credentials,
    mail: &SentMail<'_>,
) -> Result<i64, OdooError> {
    let kwargs = LogSentMail {
        body: mail.body,
        subject: mail.subject,
        message_type: "email",
        subtype_xmlid: "mail.mt_note",
        partner_ids: [],
        email_from: mail.email_from,
        reply_to: mail.reply_to,
        rfc5322_id: mail.rfc5322_id,
    };

    let kwargs = serde_json::to_value(&kwargs).map_err(|e| {
        OdooError::Internal(format!("could not serialize the chatter payload: {e}"))
    })?;

    let result = client
        .execute_kw(
            creds,
            ModelCall {
                model: mail.res_model,
                method: "message_post",
                // JSON: protocol boundary — Odoo's execute_kw positional args.
                args: serde_json::json!([[mail.res_id]]),
                kwargs,
            },
        )
        .await?;

    Ok(result.as_i64().unwrap_or_default())
}
