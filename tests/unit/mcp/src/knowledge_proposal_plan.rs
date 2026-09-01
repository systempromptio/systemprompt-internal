//! The planner's decision table, the selection rule an approver's unticks
//! must satisfy, and the label a dashboard prints for each action.

use chrono::NaiveDate;
use systemprompt_mcp_knowledge_bank::proposal::intent::{CrmIntent, Disposition, IntentTask};
use systemprompt_mcp_knowledge_bank::proposal::lookup::{
    LeadRef, OdooCapabilities, OdooLookup, PartnerRef,
};
use systemprompt_mcp_knowledge_bank::proposal::plan::{
    PlanInput, PlanOutcome, SelectionError, SkipReason, plan, validate_selection,
};
use systemprompt_mcp_knowledge_bank::proposal::{ActionTarget, OdooAction, Sender};

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
    }
}

fn task(due: Option<&str>) -> IntentTask {
    IntentTask {
        title: "Send the tier sheet".to_owned(),
        due_date: due.map(str::to_owned),
        detail: "Enterprise tier, annual pricing.".to_owned(),
    }
}

fn sender() -> Sender {
    Sender {
        name: Some("Victor".to_owned()),
        email: "victor@acme.example".to_owned(),
    }
}

struct Case {
    category: &'static str,
    intent: CrmIntent,
    lookup: OdooLookup,
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
        project: false,
        task_project: None,
    }
}

fn lead_found() -> OdooLookup {
    OdooLookup {
        partner: Some(PartnerRef {
            id: 7,
            name: "Acme".to_owned(),
        }),
        lead: Some(LeadRef {
            id: 42,
            name: "Acme renewal".to_owned(),
            partner_id: Some(7),
        }),
    }
}

fn partner_only() -> OdooLookup {
    OdooLookup {
        partner: Some(PartnerRef {
            id: 7,
            name: "Acme".to_owned(),
        }),
        lead: None,
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
fn noise_and_internal_dispositions_skip() {
    assert_eq!(
        run(&base(Disposition::Noise, lead_found())),
        PlanOutcome::Skip(SkipReason::NoiseDisposition)
    );
    assert_eq!(
        run(&base(Disposition::Internal, lead_found())),
        PlanOutcome::Skip(SkipReason::InternalDisposition)
    );
}

#[test]
fn an_open_lead_wins_regardless_of_disposition() {
    for disposition in [Disposition::Opportunity, Disposition::ExistingRelationship] {
        let PlanOutcome::Propose(actions) = run(&base(disposition, lead_found())) else {
            panic!("a found lead is proposed against");
        };
        assert!(matches!(
            &actions[0],
            OdooAction::PostChatter { target: ActionTarget::Existing { model, res_id: 42, .. }, .. } if model == "crm.lead"
        ));
        assert!(
            matches!(&actions[1], OdooAction::CreateActivity { date_deadline, .. } if date_deadline == "2026-09-10")
        );
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, OdooAction::CreateLead { .. }))
        );
    }
}

#[test]
fn an_opportunity_from_a_known_partner_creates_a_lead_linked_to_them() {
    let PlanOutcome::Propose(actions) = run(&base(Disposition::Opportunity, partner_only())) else {
        panic!("opportunity is proposed");
    };
    assert!(matches!(
        &actions[0],
        OdooAction::CreateLead { partner_id: Some(7), email_from, title, .. }
            if email_from == "victor@acme.example" && title == "Acme — pricing enquiry"
    ));
    assert!(matches!(
        &actions[1],
        OdooAction::PostChatter {
            target: ActionTarget::CreatedLead { action_index: 0 },
            ..
        }
    ));
    assert_eq!(actions[2].depends_on(), Some(0));
}

#[test]
fn an_existing_relationship_with_a_partner_logs_on_the_partner() {
    let PlanOutcome::Propose(actions) =
        run(&base(Disposition::ExistingRelationship, partner_only()))
    else {
        panic!("relationship is proposed");
    };
    assert!(matches!(
        &actions[0],
        OdooAction::PostChatter { target: ActionTarget::Existing { model, res_id: 7, .. }, .. } if model == "res.partner"
    ));
}

#[test]
fn an_unknown_sender_is_a_lead_only_when_it_is_an_opportunity() {
    let PlanOutcome::Propose(actions) = run(&base(Disposition::Opportunity, OdooLookup::default()))
    else {
        panic!("a new opportunity is proposed");
    };
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
    let PlanOutcome::Propose(actions) = run(&case) else {
        panic!("proposed");
    };
    assert!(
        matches!(&actions[1], OdooAction::CreateTask { project, date_deadline: Some(d), .. } if project == "Sales follow-ups" && d == "2026-09-10")
    );

    case.task_project = None;
    let PlanOutcome::Propose(actions) = run(&case) else {
        panic!("proposed");
    };
    assert!(
        matches!(&actions[1], OdooAction::CreateActivity { .. }),
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
    let PlanOutcome::Propose(actions) = run(&case) else {
        panic!("proposed");
    };
    for action in &actions[1..] {
        assert!(
            matches!(action, OdooAction::CreateActivity { date_deadline, .. } if date_deadline == "2026-09-08")
        );
    }
}

#[test]
fn an_empty_lead_title_falls_back_to_the_subject() {
    let mut case = base(Disposition::Opportunity, OdooLookup::default());
    case.intent.lead_title = Some("   ".to_owned());
    let PlanOutcome::Propose(actions) = run(&case) else {
        panic!("proposed");
    };
    assert!(matches!(&actions[0], OdooAction::CreateLead { title, .. } if title == "Pricing?"));
}

#[test]
fn selection_rejects_dropping_a_lead_that_follow_ups_depend_on() {
    let PlanOutcome::Propose(actions) = run(&base(Disposition::Opportunity, OdooLookup::default()))
    else {
        panic!("proposed");
    };
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
}

#[test]
fn labels_read_as_a_sentence_an_approver_can_act_on() {
    let PlanOutcome::Propose(actions) = run(&base(Disposition::Opportunity, lead_found())) else {
        panic!("proposed");
    };
    assert_eq!(actions[0].label(), "Log the email on lead #42 Acme renewal");
    assert_eq!(
        actions[1].label(),
        "Schedule \u{201c}Send the tier sheet\u{201d} on lead #42 Acme renewal"
    );
    assert_eq!(actions[0].kind(), "post_chatter");
}
