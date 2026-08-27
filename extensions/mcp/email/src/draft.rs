//! The draft: its typed shape, its validation, and how it is previewed.
//!
//! Per repository rule 8 the wire shape is a `#[derive(Serialize,
//! Deserialize)]` struct, not a `json!` literal or a `.get()` chain over a
//! `Value`.

use rmcp::model::{
    ElicitResult, ElicitationAction, ElicitationSchema, InputRequest, InputRequests, InputResponses,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt::models::artifacts::{
    CardCta, CardSection, CliArtifact, CtaVariant, PresentationCardArtifact,
};

use crate::error::EmailToolError;

// Why: The key under which the confirm round's elicitation is carried, in both
// `inputRequests` and the client's `inputResponses`.
pub const APPROVE_KEY: &str = "approve_send";

// Why: The boolean the human must set for the send to proceed.
pub const CONFIRM_FIELD: &str = "confirm";

/// `email_send`'s arguments.
///
/// Field names are the wire contract: the governance `require_approval` rule
/// conditions on `to` by path, so renaming it silently removes the
/// external-recipient hold rather than failing loudly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SendEmailInput {
    // Why: Recipients, as RFC5322 addresses.
    pub to: Vec<String>,
    // Why: Subject line.
    pub subject: String,
    // Why: Plain-text body.
    pub body: String,
    // Why: Where replies should go. Never becomes the `From:` — see
    // `EmailService::send_plain`'s note on SPF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    // Why: Optional Odoo anchor: the model to log the sent mail against, e.g.
    // `crm.lead`. Both this and `res_id` must be present for a write-back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_model: Option<String>,
    // Why: Optional Odoo anchor: the record id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_id: Option<i64>,
}

impl SendEmailInput {
    // Why: Rejects anything unsendable before a draft is ever shown.
    //
    // [`EmailToolError::Invalid`] describing the first problem found.
    pub fn validate(&self) -> Result<(), EmailToolError> {
        if self.to.is_empty() {
            return Err(EmailToolError::Invalid(
                "At least one recipient is required.".to_owned(),
            ));
        }
        for address in &self.to {
            if !is_plausible_address(address) {
                return Err(EmailToolError::Invalid(format!(
                    "{address:?} is not a valid email address."
                )));
            }
        }
        if let Some(reply_to) = &self.reply_to
            && !is_plausible_address(reply_to)
        {
            return Err(EmailToolError::Invalid(format!(
                "reply_to {reply_to:?} is not a valid email address."
            )));
        }
        if self.subject.trim().is_empty() {
            return Err(EmailToolError::Invalid("A subject is required.".to_owned()));
        }
        if self.body.trim().is_empty() {
            return Err(EmailToolError::Invalid("A body is required.".to_owned()));
        }
        // Why: half an anchor is always a mistake, and silently skipping the
        // write-back would leave the caller believing the record was updated.
        match (&self.res_model, self.res_id) {
            (Some(_), None) => Err(EmailToolError::Invalid(
                "res_model was given without res_id; both are required to log the mail on an Odoo \
                 record."
                    .to_owned(),
            )),
            (None, Some(_)) => Err(EmailToolError::Invalid(
                "res_id was given without res_model; both are required to log the mail on an Odoo \
                 record."
                    .to_owned(),
            )),
            _ => Ok(()),
        }
    }

    // Why: The Odoo anchor, if this draft carries a complete one.
    #[must_use]
    pub const fn anchor(&self) -> Option<(&str, i64)> {
        match (&self.res_model, self.res_id) {
            (Some(model), Some(id)) => Some((model.as_str(), id)),
            _ => None,
        }
    }

    // Why: The draft as a preview card — what the human actually looks at.
    #[must_use]
    pub fn preview_card(&self) -> CliArtifact {
        let mut card = PresentationCardArtifact::new("Email draft")
            .with_subtitle(self.subject.clone())
            .add_section(CardSection::new("To", self.to.join(", ")));

        if let Some(reply_to) = &self.reply_to {
            card = card.add_section(CardSection::new("Reply-To", reply_to.clone()));
        }
        card = card.add_section(CardSection::new("Subject", self.subject.clone()));
        card = card.add_section(CardSection::new("Body", self.body.clone()));

        if let Some((model, id)) = self.anchor() {
            card = card.add_section(CardSection::new(
                "Will be logged on",
                format!("{model} #{id}"),
            ));
        } else {
            card = card.add_section(CardSection::new(
                "Will be logged on",
                "Nothing — this draft has no Odoo record anchor, so no chatter entry will be \
                 written.",
            ));
        }

        // Why: these are advisory. A card CTA calls
        // `McpAppBridge.sendMessage`, which puts a prompt in front of the
        // model — it does not call the tool and it is not the approval. The
        // gate is the elicitation below and the governance hold behind it.
        card = card.with_ctas(vec![
            CardCta::new(
                "send",
                "Looks right",
                format!("Approve and send the email draft to {}", self.to.join(", ")),
                CtaVariant::Primary,
            ),
            CardCta::new(
                "discard",
                "Discard",
                "Discard that email draft; do not send it.",
                CtaVariant::Secondary,
            ),
        ]);

        CliArtifact::presentation_card(card)
    }

    // Why: The draft rendered as text, for the elicitation message.
    //
    // This is not decoration: a client with no artifact rendering shows only
    // this string, and a human must never be asked to approve a send whose
    // contents they cannot see.
    #[must_use]
    pub fn as_plain_text(&self) -> String {
        let mut out = format!("To: {}\n", self.to.join(", "));
        if let Some(reply_to) = &self.reply_to {
            out.push_str(&format!("Reply-To: {reply_to}\n"));
        }
        out.push_str(&format!("Subject: {}\n", self.subject));
        if let Some((model, id)) = self.anchor() {
            out.push_str(&format!("Will be logged on: {model} #{id}\n"));
        }
        out.push('\n');
        out.push_str(&self.body);
        out
    }

    // Why: The single-entry `inputRequests` map for the confirm round.
    //
    // [`EmailToolError::Internal`] if the confirmation schema will not build.
    // That is a programming error in the literal below, not anything the
    // caller did, but it is not worth a panic on a mail server.
    pub fn approval_request(&self) -> Result<InputRequests, EmailToolError> {
        let schema = ElicitationSchema::builder()
            .required_bool(CONFIRM_FIELD)
            .build()
            .map_err(|e| {
                EmailToolError::Internal(format!("could not build the confirmation schema: {e}"))
            })?;

        let message = format!(
            "Send this email?\n\n{}\n\nSet {CONFIRM_FIELD} to true to send it, or decline to \
             discard it.",
            self.as_plain_text()
        );

        let mut requests = InputRequests::new();
        requests.insert(
            APPROVE_KEY.to_owned(),
            InputRequest::Elicitation(rmcp::model::ElicitRequest::new(
                rmcp::model::ElicitRequestParams::FormElicitationParams {
                    meta: None,
                    message,
                    requested_schema: schema,
                },
            )),
        );
        Ok(requests)
    }
}

// Why: A deliberately conservative shape check.
//
// Full RFC5322 validation belongs to the transport, which parses into a
// `Mailbox` and errors precisely. This exists so an obvious typo is caught
// before a human is asked to approve it, not to be authoritative.
fn is_plausible_address(address: &str) -> bool {
    let trimmed = address.trim();
    let addr_spec = trimmed
        .rfind('<')
        .zip(trimmed.rfind('>'))
        .and_then(|(open, close)| (open < close).then(|| &trimmed[open + 1..close]))
        .unwrap_or(trimmed);

    let mut parts = addr_spec.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !addr_spec.contains(char::is_whitespace)
}

/// What the client said about sending, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    // Why: No confirm round has happened yet — this is round one.
    NotAsked,
    // Why: A human accepted, with `confirm: true`.
    Confirmed,
    // Why: A human declined, or said something we cannot read as a yes.
    Declined,
}

// Why: Reads the confirm round's answer out of `inputResponses`.
//
// Everything that is not an explicit accept carrying `confirm: true` is a
// decline, including a response that will not parse. The only safe reading of
// "I could not understand the human's answer" is that they did not say yes,
// and this is the single function standing between a draft and a real email.
#[must_use]
pub fn confirmation(responses: Option<&InputResponses>) -> Confirmation {
    let Some(responses) = responses else {
        return Confirmation::NotAsked;
    };
    // Why: A retry that carries responses but not OURS has not answered the
    // question we asked, so it is round one again rather than a decline.
    let Some(raw) = responses.get(APPROVE_KEY) else {
        return Confirmation::NotAsked;
    };

    let Ok(result) = serde_json::from_value::<ElicitResult>(raw.clone()) else {
        tracing::warn!("could not parse the confirmation response; treating it as a decline");
        return Confirmation::Declined;
    };

    if !matches!(result.action, ElicitationAction::Accept) {
        return Confirmation::Declined;
    }

    let confirmed = result
        .content
        .as_ref()
        .and_then(|content| content.get(CONFIRM_FIELD))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    if confirmed {
        Confirmation::Confirmed
    } else {
        Confirmation::Declined
    }
}
