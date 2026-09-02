//! The planner's decision table, the selection rule an approver's unticks
//! must satisfy, and the label a dashboard prints for each action.

use std::collections::HashMap;

use chrono::NaiveDate;
use systemprompt_mcp_knowledge_bank::proposal::intent::{
    CrmIntent, DealStageHint, Disposition, IntentTask, category_tag,
};
use systemprompt_mcp_knowledge_bank::proposal::lookup::{
    LeadRef, OdooCapabilities, OdooLookup, PartnerRef, UserRef, assignee_key,
};
use systemprompt_mcp_knowledge_bank::proposal::plan::{
    PlanInput, PlanOutcome, SelectionError, SkipReason, plan, validate_selection,
};
use systemprompt_mcp_knowledge_bank::proposal::{ActionTarget, Assignee, OdooAction, Sender};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 1).expect("valid date")
}

fn intent(disposition: Disposition, tasks: Vec<IntentTask>) -> CrmIntent {
    CrmIntent {
        disposition,
        lead_title: Some("Acme — pricing enquiry".to_owned()),
        contact_name: Some("Victor Acme".to_owned()),
        company_name: Some("Acme".to_owned()),
        note_summary: "Victor asks for the enterprise tier sheet.".to_owned(),
        tasks,
        confidence: 0.9,
        deal_stage_hint: None,
        expected_close_date: None,
        expected_revenue: None,
    }
}

fn task(due: Option<&str>) -> IntentTask {
    IntentTask {
        title: "Send the tier sheet".to_owned(),
        due_date: due.map(str::to_owned),
        detail: "Enterprise tier, annual pricing.".to_owned(),
        assignee: None,
    }
}

fn task_for(assignee: &str) -> IntentTask {
    IntentTask {
        assignee: Some(assignee.to_owned()),
        ..task(Some("2026-09-10"))
    }
}

fn sender() -> Sender {
    Sender {
        name: Some("Victor".to_owned()),
        email: "victor@acme.example".to_owned(),
    }
}

fn colleagues() -> HashMap<String, UserRef> {
    HashMap::from([(
        assignee_key("Sam Ops"),
        UserRef {
            id: 12,
            name: "Sam Ops".to_owned(),
        },
    )])
}

struct Case {
    category: &'static str,
    intent: CrmIntent,
    lookup: OdooLookup,
    assignees: HashMap<String, UserRef>,
    project: bool,
    task_project: Option<&'static str>,
}

fn run(case: &Case) -> PlanOutcome {
    let sender = sender();
    plan(&PlanInput {
        category: case.category,
        subject: "Pricing?",
        intent: &case.intent,
        sender: &sender,
        lookup: &case.lookup,
        assignees: &case.assignees,
        capabilities: OdooCapabilities {
            project: case.project,
        },
        task_project: case.task_project,
        today: today(),
    })
}

fn base(disposition: Disposition, lookup: OdooLookup) -> Case {
    Case {
        category: "sales",
        intent: intent(disposition, vec![task(Some("2026-09-10"))]),
        lookup,
        assignees: HashMap::new(),
        project: false,
        task_project: None,
    }
}

fn acme() -> PartnerRef {
    PartnerRef {
        id: 7,
        name: "Acme".to_owned(),
    }
}

fn lead_found() -> OdooLookup {
    OdooLookup {
        partner: Some(acme()),
        lead: Some(LeadRef {
            id: 42,
            name: "Acme renewal".to_owned(),
            partner_id: Some(7),
        }),
        owner_partner: None,
    }
}

fn partner_only() -> OdooLookup {
    OdooLookup {
        partner: Some(acme()),
        lead: None,
        owner_partner: None,
    }
}

fn with_owner(mut lookup: OdooLookup) -> OdooLookup {
    lookup.owner_partner = Some(PartnerRef {
        id: 3,
        name: "Ed Burton".to_owned(),
    });
    lookup
}

fn proposed(case: &Case) -> Vec<OdooAction> {
    match run(case) {
        PlanOutcome::Propose(actions) => actions,
        PlanOutcome::Skip(reason) => panic!("expected a proposal, got skip {reason:?}"),
    }
}

#[test]
fn noise_categories_skip_before_anything_else_is_consulted() {
    for category in ["spam", "newsletter", "notification"] {
        let mut case = base(Disposition::Opportunity, lead_found());
        case.category = category;
        assert_eq!(
            run(&case),
            PlanOutcome::Skip(SkipReason::NoiseCategory),
            "{category}"
        );
    }
}

#[test]
fn noise_and_taskless_internal_dispositions_skip() {
    assert_eq!(
        run(&base(Disposition::Noise, lead_found())),
        PlanOutcome::Skip(SkipReason::NoiseDisposition)
    );
    let mut internal = base(Disposition::Internal, with_owner(lead_found()));
    internal.intent.tasks.clear();
    assert_eq!(
        run(&internal),
        PlanOutcome::Skip(SkipReason::InternalDisposition),
        "an internal thread that delegates nothing has nothing to land"
    );
    assert_eq!(
        run(&base(Disposition::Internal, lead_found())),
        PlanOutcome::Skip(SkipReason::InternalDisposition),
        "without the owner's own contact there is nowhere to anchor"
    );
}

#[test]
fn an_internal_thread_with_tasks_lands_them_on_the_owner_without_logging_the_body() {
    let mut case = base(Disposition::Internal, with_owner(lead_found()));
    case.intent.tasks = vec![task_for("sam ops"), task(None)];
    case.assignees = colleagues();
    let actions = proposed(&case);
    assert_eq!(actions.len(), 2, "follow-ups only: {actions:?}");
    assert!(actions.iter().all(|a| matches!(
        a,
        OdooAction::CreateActivity { target: ActionTarget::Existing { model, res_id: 3, .. }, .. } if model == "res.partner"
    )));
    assert!(
        !actions.iter().any(|a| matches!(
            a,
            OdooAction::PostChatter { .. } | OdooAction::TagRecord { .. }
        )),
        "internal bodies stay out of Odoo and carry no tag"
    );
    assert!(matches!(
        &actions[0],
        OdooAction::CreateActivity { assignee: Some(Assignee { user_id: 12, name }), .. } if name == "Sam Ops"
    ));
    assert!(matches!(
        &actions[1],
        OdooAction::CreateActivity { assignee: None, .. }
    ));
}

#[test]
fn an_open_lead_wins_regardless_of_disposition_and_is_tagged() {
    for disposition in [Disposition::Opportunity, Disposition::ExistingRelationship] {
        let actions = proposed(&base(disposition, lead_found()));
        assert!(matches!(
            &actions[0],
            OdooAction::PostChatter { target: ActionTarget::Existing { model, res_id: 42, .. }, .. } if model == "crm.lead"
        ));
        assert!(matches!(
            &actions[1],
            OdooAction::TagRecord { target: ActionTarget::Existing { model, res_id: 42, .. }, tag } if model == "crm.lead" && tag == "Sales"
        ));
        assert!(
            matches!(&actions[2], OdooAction::CreateActivity { date_deadline, .. } if date_deadline == "2026-09-10")
        );
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, OdooAction::CreateLead { .. }))
        );
    }
}

#[test]
fn an_opportunity_from_a_known_partner_creates_a_tagged_lead_linked_to_them() {
    let actions = proposed(&base(Disposition::Opportunity, partner_only()));
    assert!(matches!(
        &actions[0],
        OdooAction::CreateLead { partner_id: Some(7), email_from, title, tags, .. }
            if email_from == "victor@acme.example" && title == "Acme — pricing enquiry" && tags == &["Sales".to_owned()]
    ));
    assert!(matches!(
        &actions[1],
        OdooAction::PostChatter {
            target: ActionTarget::CreatedLead { action_index: 0 },
            ..
        }
    ));
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, OdooAction::TagRecord { .. })),
        "a created lead carries its tag inline"
    );
    assert_eq!(actions[2].depends_on(), Some(0));
}

#[test]
fn an_existing_relationship_with_a_partner_logs_on_and_tags_the_partner() {
    let mut case = base(Disposition::ExistingRelationship, partner_only());
    case.category = "client";
    let actions = proposed(&case);
    assert!(matches!(
        &actions[0],
        OdooAction::PostChatter { target: ActionTarget::Existing { model, res_id: 7, .. }, .. } if model == "res.partner"
    ));
    assert!(matches!(
        &actions[1],
        OdooAction::TagRecord { target: ActionTarget::Existing { model, res_id: 7, .. }, tag } if model == "res.partner" && tag == "Client"
    ));
}

#[test]
fn a_legal_opportunity_becomes_a_lead_tagged_legal() {
    let mut case = base(Disposition::Opportunity, partner_only());
    case.category = "legal";
    let actions = proposed(&case);
    assert!(matches!(
        &actions[0],
        OdooAction::CreateLead { tags, partner_id: Some(7), .. } if tags == &["Legal".to_owned()]
    ));
}

#[test]
fn other_and_noise_categories_carry_no_tag() {
    for category in ["other", "spam", "newsletter", "notification", "galactic"] {
        assert_eq!(category_tag(category), None, "{category}");
    }
    let mut case = base(Disposition::ExistingRelationship, partner_only());
    case.category = "other";
    let actions = proposed(&case);
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, OdooAction::TagRecord { .. }))
    );
    assert!(
        matches!(&actions[1], OdooAction::CreateActivity { .. }),
        "follow-ups directly after the log when there is no tag"
    );

    let mut case = base(Disposition::Opportunity, OdooLookup::default());
    case.category = "other";
    let actions = proposed(&case);
    assert!(matches!(&actions[0], OdooAction::CreateLead { tags, .. } if tags.is_empty()));
}

#[test]
fn deal_fields_ride_the_created_lead_and_a_past_close_date_is_dropped() {
    let mut case = base(Disposition::Opportunity, OdooLookup::default());
    case.intent.deal_stage_hint = Some(DealStageHint::Proposition);
    case.intent.expected_close_date = Some("2026-10-15".to_owned());
    case.intent.expected_revenue = Some(12_500.0);
    let actions = proposed(&case);
    assert!(matches!(
        &actions[0],
        OdooAction::CreateLead { stage_hint: Some(stage), date_deadline: Some(close), expected_revenue: Some(revenue), .. }
            if stage == "Proposition" && close == "2026-10-15" && (*revenue - 12_500.0).abs() < f64::EPSILON
    ));

    case.intent.expected_close_date = Some("2020-01-01".to_owned());
    case.intent.expected_revenue = Some(0.0);
    let actions = proposed(&case);
    assert!(matches!(
        &actions[0],
        OdooAction::CreateLead {
            date_deadline: None,
            expected_revenue: None,
            ..
        }
    ));
}

#[test]
fn an_assignee_resolves_case_insensitively_and_an_unknown_one_falls_back() {
    let mut case = base(Disposition::Opportunity, lead_found());
    case.intent.tasks = vec![task_for("  SAM OPS "), task_for("nobody@nowhere.example")];
    case.assignees = colleagues();
    let actions = proposed(&case);
    assert!(matches!(
        &actions[2],
        OdooAction::CreateActivity {
            assignee: Some(Assignee { user_id: 12, .. }),
            ..
        }
    ));
    assert!(
        matches!(
            &actions[3],
            OdooAction::CreateActivity { assignee: None, .. }
        ),
        "unresolved → the approver keeps it"
    );
}

#[test]
fn an_unknown_sender_is_a_lead_only_when_it_is_an_opportunity() {
    let actions = proposed(&base(Disposition::Opportunity, OdooLookup::default()));
    assert!(matches!(
        &actions[0],
        OdooAction::CreateLead {
            partner_id: None,
            ..
        }
    ));
    assert_eq!(
        run(&base(
            Disposition::ExistingRelationship,
            OdooLookup::default()
        )),
        PlanOutcome::Skip(SkipReason::NoOdooAnchor)
    );
}

#[test]
fn follow_ups_become_tasks_only_with_project_installed_and_configured() {
    let mut case = base(Disposition::Opportunity, lead_found());
    case.project = true;
    case.task_project = Some("Sales follow-ups");
    case.intent.tasks = vec![task_for("Sam Ops")];
    case.assignees = colleagues();
    let actions = proposed(&case);
    assert!(
        matches!(&actions[2], OdooAction::CreateTask { project, date_deadline: Some(d), assignee: Some(Assignee { user_id: 12, .. }), .. } if project == "Sales follow-ups" && d == "2026-09-10")
    );

    case.task_project = None;
    let actions = proposed(&case);
    assert!(
        matches!(&actions[2], OdooAction::CreateActivity { .. }),
        "no project name → activity"
    );
}

#[test]
fn a_past_or_unparseable_due_date_falls_back_to_a_week_out() {
    let mut case = base(Disposition::Opportunity, lead_found());
    case.intent = intent(
        Disposition::Opportunity,
        vec![task(Some("2020-01-01")), task(Some("soon"))],
    );
    let actions = proposed(&case);
    for action in &actions[2..] {
        assert!(
            matches!(action, OdooAction::CreateActivity { date_deadline, .. } if date_deadline == "2026-09-08")
        );
    }
}

#[test]
fn an_empty_lead_title_falls_back_to_the_subject() {
    let mut case = base(Disposition::Opportunity, OdooLookup::default());
    case.intent.lead_title = Some("   ".to_owned());
    let actions = proposed(&case);
    assert!(matches!(&actions[0], OdooAction::CreateLead { title, .. } if title == "Pricing?"));
}

#[test]
fn selection_rejects_dropping_a_lead_that_follow_ups_depend_on() {
    let actions = proposed(&base(Disposition::Opportunity, OdooLookup::default()));
    assert_eq!(validate_selection(&actions, &[]), Ok(vec![0, 1, 2]));
    assert_eq!(validate_selection(&actions, &[2]), Ok(vec![0, 1]));
    assert_eq!(
        validate_selection(&actions, &[0]),
        Err(SelectionError::BrokenDependency {
            dependent: 1,
            dependency: 0
        })
    );
    assert_eq!(
        validate_selection(&actions, &[0, 1, 2]),
        Err(SelectionError::Empty)
    );
    assert_eq!(
        validate_selection(&actions, &[9]),
        Err(SelectionError::OutOfRange(9))
    );

    let actions = proposed(&base(Disposition::ExistingRelationship, lead_found()));
    assert_eq!(
        validate_selection(&actions, &[1]),
        Ok(vec![0, 2]),
        "a tag on an existing record can be unticked on its own"
    );
}

#[test]
fn labels_read_as_a_sentence_an_approver_can_act_on() {
    let mut case = base(Disposition::Opportunity, lead_found());
    case.intent.tasks.push(task_for("Sam Ops"));
    case.assignees = colleagues();
    let actions = proposed(&case);
    assert_eq!(actions[0].label(), "Log the email on lead #42 Acme renewal");
    assert_eq!(
        actions[1].label(),
        "Tag lead #42 Acme renewal \u{201c}Sales\u{201d}"
    );
    assert_eq!(
        actions[2].label(),
        "Schedule \u{201c}Send the tier sheet\u{201d} on lead #42 Acme renewal"
    );
    assert_eq!(
        actions[3].label(),
        "Schedule \u{201c}Send the tier sheet\u{201d} on lead #42 Acme renewal for Sam Ops"
    );
    assert_eq!(actions[0].kind(), "post_chatter");
    assert_eq!(actions[1].kind(), "tag_record");
}
