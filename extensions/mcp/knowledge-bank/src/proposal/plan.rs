//! The pure planner: intent + what Odoo already holds → the actions to
//! propose. No I/O, so every branch of the decision table is assertable.
//!
//! The table, in priority order:
//!
//! | category / disposition            | Odoo already has | proposal                                            |
//! |-----------------------------------|------------------|-----------------------------------------------------|
//! | spam, newsletter, notification    | —                | skip                                                |
//! | `noise`                           | —                | skip                                                |
//! | `internal`, no tasks              | —                | skip                                                |
//! | `internal`, tasks                 | owner's partner  | follow-ups on the owner's own contact; no log, no tag |
//! | anything else                     | an open lead     | log on the lead + tag + follow-ups on the lead       |
//! | `opportunity`                     | a partner        | new tagged lead for that partner + log + follow-ups  |
//! | `existing_relationship`           | a partner        | log on the partner + tag + follow-ups                |
//! | `opportunity`                     | nothing          | new tagged lead + log + follow-ups                   |
//! | `existing_relationship`           | nothing          | skip (no record to anchor to)                        |
//!
//! The category becomes a tag: inline on a lead this proposal creates, a
//! `tag_record` action on an anchor that already exists, nothing for `other`
//! or the noise categories. Deal fields (`stage_hint`, `date_deadline`,
//! `expected_revenue`) ride only on a created lead. Follow-ups become
//! `project.task`s only when Project is installed *and* a project was
//! configured; otherwise they are `mail.activity`s on the record, which every
//! Odoo has. A follow-up whose assignee the lookup resolved is handed to that
//! colleague; otherwise the approver keeps it.
//!
//! Action order is `[create_lead?], post_chatter, tag_record?, follow-ups…`, so
//! a follow-up's dependency on the created lead is always index 0.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::intent::{CrmIntent, Disposition, IntentTask, NOISE_CATEGORIES, category_tag};
use super::lookup::{OdooCapabilities, OdooLookup, UserRef, assignee_key};
use super::sender::Sender;
use super::{ActionTarget, Assignee, OdooAction};

const DEFAULT_FOLLOW_UP_DAYS: i64 = 7;

/// Everything the planner looks at, borrowed.
#[derive(Debug, Clone, Copy)]
pub struct PlanInput<'a> {
    pub category: &'a str,
    pub subject: &'a str,
    pub intent: &'a CrmIntent,
    pub sender: &'a Sender,
    pub lookup: &'a OdooLookup,
    pub assignees: &'a HashMap<String, UserRef>,
    pub capabilities: OdooCapabilities,
    pub task_project: Option<&'a str>,
    pub today: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    NoiseCategory,
    NoiseDisposition,
    InternalDisposition,
    NoOdooAnchor,
}

impl SkipReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoiseCategory => "noise_category",
            Self::NoiseDisposition => "noise_disposition",
            Self::InternalDisposition => "internal_disposition",
            Self::NoOdooAnchor => "no_odoo_anchor",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanOutcome {
    Skip(SkipReason),
    Propose(Vec<OdooAction>),
}

#[must_use]
pub fn plan(input: &PlanInput<'_>) -> PlanOutcome {
    if NOISE_CATEGORIES.contains(&input.category) {
        return PlanOutcome::Skip(SkipReason::NoiseCategory);
    }
    match input.intent.disposition {
        Disposition::Noise => return PlanOutcome::Skip(SkipReason::NoiseDisposition),
        Disposition::Internal => return plan_internal(input),
        Disposition::Opportunity | Disposition::ExistingRelationship => {},
    }

    let mut actions = Vec::new();
    let anchor = if let Some(lead) = &input.lookup.lead {
        ActionTarget::Existing {
            model: "crm.lead".to_owned(),
            res_id: lead.id,
            label: format!("lead #{} {}", lead.id, lead.name),
        }
    } else if input.intent.disposition == Disposition::Opportunity {
        actions.push(create_lead(input));
        ActionTarget::CreatedLead { action_index: 0 }
    } else if let Some(partner) = &input.lookup.partner {
        ActionTarget::Existing {
            model: "res.partner".to_owned(),
            res_id: partner.id,
            label: format!("contact {}", partner.name),
        }
    } else {
        return PlanOutcome::Skip(SkipReason::NoOdooAnchor);
    };

    actions.push(OdooAction::PostChatter {
        target: anchor.clone(),
        subject: input.subject.to_owned(),
    });
    if matches!(anchor, ActionTarget::Existing { .. })
        && let Some(tag) = category_tag(input.category)
    {
        actions.push(OdooAction::TagRecord {
            target: anchor.clone(),
            tag: tag.to_owned(),
        });
    }
    for task in &input.intent.tasks {
        actions.push(follow_up(input, task, &anchor));
    }
    PlanOutcome::Propose(actions)
}

// Why: an internal thread's body stays out of Odoo — colleagues already have
// it — but the work it delegates still has to land on someone's list.
fn plan_internal(input: &PlanInput<'_>) -> PlanOutcome {
    if input.intent.tasks.is_empty() {
        return PlanOutcome::Skip(SkipReason::InternalDisposition);
    }
    let Some(owner) = &input.lookup.owner_partner else {
        return PlanOutcome::Skip(SkipReason::InternalDisposition);
    };
    let anchor = ActionTarget::Existing {
        model: "res.partner".to_owned(),
        res_id: owner.id,
        label: format!("contact {}", owner.name),
    };
    PlanOutcome::Propose(
        input
            .intent
            .tasks
            .iter()
            .map(|task| follow_up(input, task, &anchor))
            .collect(),
    )
}

fn create_lead(input: &PlanInput<'_>) -> OdooAction {
    let title = input
        .intent
        .lead_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map_or_else(|| input.subject.to_owned(), str::to_owned);
    OdooAction::CreateLead {
        title,
        contact_name: input
            .intent
            .contact_name
            .clone()
            .or_else(|| input.sender.name.clone()),
        partner_name: input.intent.company_name.clone(),
        email_from: input.sender.email.clone(),
        partner_id: input.lookup.partner.as_ref().map(|p| p.id),
        description: input.intent.note_summary.clone(),
        stage_hint: input
            .intent
            .deal_stage_hint
            .map(|h| h.odoo_stage_name().to_owned()),
        date_deadline: future_date(input.intent.expected_close_date.as_deref(), input.today)
            .map(|d| d.to_string()),
        expected_revenue: input
            .intent
            .expected_revenue
            .filter(|r| r.is_finite() && *r > 0.0),
        tags: category_tag(input.category)
            .map(str::to_owned)
            .into_iter()
            .collect(),
    }
}

fn future_date(raw: Option<&str>, today: NaiveDate) -> Option<NaiveDate> {
    raw.and_then(|d| NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").ok())
        .filter(|d| *d >= today)
}

fn follow_up(input: &PlanInput<'_>, task: &IntentTask, anchor: &ActionTarget) -> OdooAction {
    let deadline = future_date(task.due_date.as_deref(), input.today);
    let assignee = task
        .assignee
        .as_deref()
        .and_then(|name| input.assignees.get(&assignee_key(name)))
        .map(|user| Assignee {
            user_id: user.id,
            name: user.name.clone(),
        });
    input
        .task_project
        .filter(|_| input.capabilities.project)
        .map_or_else(
            || OdooAction::CreateActivity {
                target: anchor.clone(),
                summary: task.title.clone(),
                note: task.detail.clone(),
                date_deadline: deadline
                    .unwrap_or(input.today + Duration::days(DEFAULT_FOLLOW_UP_DAYS))
                    .to_string(),
                assignee: assignee.clone(),
            },
            |project| OdooAction::CreateTask {
                target: anchor.clone(),
                project: project.to_owned(),
                name: task.title.clone(),
                description: task.detail.clone(),
                date_deadline: deadline.map(|d| d.to_string()),
                assignee: assignee.clone(),
            },
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    #[error("action index {0} is out of range")]
    OutOfRange(usize),
    #[error(
        "action {dependent} lands on the lead that action {dependency} creates; exclude both or neither"
    )]
    BrokenDependency { dependent: usize, dependency: usize },
    #[error("every action was excluded; reject the proposal instead")]
    Empty,
}

// Why: an admin may untick actions, but not in a way that leaves a follow-up
// pointing at a lead that will never exist.
pub fn validate_selection(
    actions: &[OdooAction],
    exclude: &[usize],
) -> Result<Vec<usize>, SelectionError> {
    if let Some(&bad) = exclude.iter().find(|&&i| i >= actions.len()) {
        return Err(SelectionError::OutOfRange(bad));
    }
    let selected: Vec<usize> = (0..actions.len())
        .filter(|i| !exclude.contains(i))
        .collect();
    if selected.is_empty() {
        return Err(SelectionError::Empty);
    }
    for &i in &selected {
        if let Some(dep) = actions[i].depends_on()
            && !selected.contains(&dep)
        {
            return Err(SelectionError::BrokenDependency {
                dependent: i,
                dependency: dep,
            });
        }
    }
    Ok(selected)
}
