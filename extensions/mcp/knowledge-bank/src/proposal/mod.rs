//! The Odoo projection of a captured email: what the pipeline proposes, what
//! an admin approves, and what lands.
//!
//! A document moves `raw → categorized → skipped | proposed`, then
//! `proposed → approved → applied | failed`, or `proposed → denied | expired`.
//! The `proposed` step opens an `approval_requests` row — the same table a
//! held MCP tool call parks on — so the human decision is governed by the
//! machinery that already exists rather than a parallel one. Nothing in this
//! module writes to Odoo before that row says `approved`, and every write runs
//! as the approver's own Odoo credential.
//!
//! - [`intent`] — the flat `crm_intent` the categorization prompt emits.
//! - [`sender`] — the display `From:` line reduced to name + address.
//! - [`lookup`] — read-only Odoo queries: is this sender already a partner or
//!   an open lead, and is Project installed.
//! - [`plan`] — the pure planner turning intent + lookup into [`OdooAction`]s.
//! - [`scan`] — the real `secret_scan` policy applied to a proposed body.
//! - [`body`] — the email rendered as chatter HTML.
//! - [`approval`] — opening the governance hold for a proposal.
//! - [`ledger`] — the per-action `knowledge_odoo_projection` rows.
//! - [`apply`] — walking a proposal's actions against the ledger.
//! - [`writes`] — the typed Odoo writes.
//! - [`settle`] — the single executor both the tool and the reconcile job call.

pub mod apply;
pub mod approval;
pub mod body;
pub mod intent;
pub mod ledger;
pub mod lookup;
pub mod plan;
pub mod scan;
pub mod sender;
pub mod settle;
pub mod writes;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use sender::Sender;

pub const RULE_BRAIN_EMAIL_INGEST: &str = "brain_email_ingest";
pub const TOOL_APPLY_PROPOSAL: &str = "odoo_apply_proposal";
pub const PROPOSAL_EXPIRY_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Where a document is in the capture → categorize → propose → apply pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Raw,
    Categorized,
    Skipped,
    Proposed,
    Approved,
    Applied,
    Failed,
    Denied,
    Expired,
}

impl DocumentStatus {
    pub const ALL: [Self; 9] = [
        Self::Raw,
        Self::Categorized,
        Self::Skipped,
        Self::Proposed,
        Self::Approved,
        Self::Applied,
        Self::Failed,
        Self::Denied,
        Self::Expired,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Categorized => "categorized",
            Self::Skipped => "skipped",
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Applied => "applied",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Expired => "expired",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }
}

/// Which Odoo record an action lands on: one that already exists, or the lead
/// an earlier action in the same proposal creates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionTarget {
    Existing {
        model: String,
        res_id: i64,
        label: String,
    },
    CreatedLead {
        action_index: usize,
    },
}

/// One Odoo write the planner proposes and an admin approves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OdooAction {
    CreateLead {
        title: String,
        contact_name: Option<String>,
        partner_name: Option<String>,
        email_from: String,
        partner_id: Option<i64>,
        description: String,
    },
    PostChatter {
        target: ActionTarget,
        subject: String,
    },
    CreateActivity {
        target: ActionTarget,
        summary: String,
        note: String,
        date_deadline: String,
    },
    CreateTask {
        target: ActionTarget,
        project: String,
        name: String,
        description: String,
        date_deadline: Option<String>,
    },
}

impl OdooAction {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::CreateLead { .. } => "create_lead",
            Self::PostChatter { .. } => "post_chatter",
            Self::CreateActivity { .. } => "create_activity",
            Self::CreateTask { .. } => "create_task",
        }
    }

    #[must_use]
    pub const fn target(&self) -> Option<&ActionTarget> {
        match self {
            Self::CreateLead { .. } => None,
            Self::PostChatter { target, .. }
            | Self::CreateActivity { target, .. }
            | Self::CreateTask { target, .. } => Some(target),
        }
    }

    #[must_use]
    pub const fn depends_on(&self) -> Option<usize> {
        match self.target() {
            Some(ActionTarget::CreatedLead { action_index }) => Some(*action_index),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::CreateLead { title, .. } => format!("Create lead \u{201c}{title}\u{201d}"),
            Self::PostChatter { target, .. } => {
                format!("Log the email on {}", target_label(target))
            },
            Self::CreateActivity {
                target, summary, ..
            } => {
                format!(
                    "Schedule \u{201c}{summary}\u{201d} on {}",
                    target_label(target)
                )
            },
            Self::CreateTask { project, name, .. } => {
                format!("Create task \u{201c}{name}\u{201d} in {project}")
            },
        }
    }
}

fn target_label(target: &ActionTarget) -> String {
    match target {
        ActionTarget::Existing { label, .. } => label.clone(),
        ActionTarget::CreatedLead { .. } => "the new lead".to_owned(),
    }
}

/// The plan for one document, at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    pub revision: i32,
    pub sender: Sender,
    pub actions: Vec<OdooAction>,
}

/// The `approval_requests.arguments` payload: exactly what an approver is
/// approving. The revision is part of it so a proposal re-opened after expiry
/// derives a fresh call id instead of rejoining the expired row.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalArguments {
    #[schemars(with = "String")]
    pub document_id: Uuid,
    pub revision: i32,
    pub proposal: Proposal,
}
