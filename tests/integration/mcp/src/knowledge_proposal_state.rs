//! The proposal state machine against live Postgres, without Odoo.
//!
//! What the unit suite cannot cover is the compare-and-set choreography
//! between the document row and its `approval_requests` row: a proposal is
//! opened, answered from the admin page (which only writes the row), and
//! settled later by the executor. The approve path here runs as an approver
//! with no `odoo_identity`, which is the failure the reconcile job must leave
//! retryable rather than fatal — and it exercises settle end to end short of
//! the Odoo call itself.

use systemprompt::identifiers::UserId;
use systemprompt::security::policy::{ApprovalRepository, ApprovalStatus, ApprovalVerdict};
use systemprompt_mcp_knowledge_bank::proposal::approval::{open_proposal_hold, proposal_call_id};
use systemprompt_mcp_knowledge_bank::proposal::settle::{SettleOutcome, settle_document};
use systemprompt_mcp_knowledge_bank::proposal::{ActionTarget, DocumentStatus, OdooAction, Proposal, Sender};
use systemprompt_mcp_knowledge_bank::schema::schema_definitions;
use systemprompt_mcp_knowledge_bank::store::KnowledgeStore;
use uuid::Uuid;

use super::common::TempDb;

const APPROVALS: &str = r"CREATE TABLE approval_requests (
    call_id TEXT PRIMARY KEY,
    tool_name TEXT NOT NULL,
    server_name TEXT NOT NULL,
    arguments JSONB NOT NULL DEFAULT '{}'::jsonb,
    args_digest TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    session_id TEXT,
    trace_id TEXT,
    rule TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','denied','expired')),
    approver_id TEXT,
    approver_username TEXT,
    decided_at TIMESTAMPTZ,
    decision_note TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
)";

const ODOO_IDENTITY: &str = r"CREATE TABLE odoo_identity (
    user_id TEXT PRIMARY KEY,
    odoo_login TEXT NOT NULL,
    odoo_uid INTEGER NOT NULL,
    odoo_api_key_encrypted TEXT NOT NULL
)";

async fn install(db: &TempDb) -> (KnowledgeStore, ApprovalRepository) {
    let pool = db.pool.pool().expect("throwaway pool");
    for definition in schema_definitions() {
        sqlx::raw_sql(sqlx::AssertSqlSafe(definition.sql.clone()))
            .execute(pool.as_ref())
            .await
            .expect("install knowledge schema");
    }
    for ddl in [APPROVALS, ODOO_IDENTITY] {
        sqlx::query(ddl).execute(pool.as_ref()).await.expect("install table");
    }
    (
        KnowledgeStore::new(db.pool.clone()),
        ApprovalRepository::new((*pool).clone()),
    )
}

async fn seed_categorized(db: &TempDb, subject: &str) -> Uuid {
    let pool = db.pool.pool().expect("pool");
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO knowledge_documents
            (title, source, content, uploaded_by, status, category, metadata, structured)
          VALUES ($1, 'email', E'From: v <v@acme.example>\n\nHello', 'admin', 'categorized', 'sales',
                  '{"message_id":"<m1@acme.example>","from":"Victor <v@acme.example>","date":"2026-09-01T09:00:00Z"}'::jsonb,
                  '{"summary":"s","entities":[],"action_items":[]}'::jsonb)
          RETURNING id"#,
    )
    .bind(subject)
    .fetch_one(pool.as_ref())
    .await
    .expect("seed document")
}

fn proposal() -> Proposal {
    Proposal {
        revision: 1,
        sender: Sender {
            name: Some("Victor".to_owned()),
            email: "v@acme.example".to_owned(),
        },
        actions: vec![
            OdooAction::CreateLead {
                title: "Acme".to_owned(),
                contact_name: None,
                partner_name: None,
                email_from: "v@acme.example".to_owned(),
                partner_id: None,
                description: String::new(),
            },
            OdooAction::PostChatter {
                target: ActionTarget::CreatedLead { action_index: 0 },
                subject: "Pricing".to_owned(),
            },
        ],
    }
}

async fn status_of(db: &TempDb, id: Uuid) -> (String, Option<String>, Option<String>) {
    let pool = db.pool.pool().expect("pool");
    sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT status, proposal_error, decided_by FROM knowledge_documents WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool.as_ref())
    .await
    .expect("status")
}

async fn propose(store: &KnowledgeStore, db: &TempDb, owner: &UserId, id: Uuid) -> String {
    let proposal = proposal();
    let call_id = proposal_call_id(owner, id, &proposal).expect("call id");
    assert!(store.set_proposed(id, &proposal, call_id.as_str()).await.expect("set_proposed"));
    let pool = db.pool.pool().expect("pool");
    open_proposal_hold(pool.as_ref(), owner, id, &proposal)
        .await
        .expect("open hold");
    call_id.as_str().to_owned()
}

macro_rules! with_db {
    ($db:ident, $body:block) => {
        let Some($db) = TempDb::create().await else {
            return;
        };
        $body
        $db.cleanup().await;
    };
}

#[tokio::test]
async fn proposing_is_a_compare_and_set_and_opens_one_approval_row() {
    with_db!(db, {
        let (store, repo) = install(&db).await;
        let owner = UserId::new("admin");
        let id = seed_categorized(&db, "Pricing?").await;

        let call_id = propose(&store, &db, &owner, id).await;
        assert_eq!(status_of(&db, id).await.0, "proposed");
        let row = repo.find(&call_id).await.expect("find").expect("row exists");
        assert_eq!(row.rule, "brain_email_ingest");
        assert_eq!(row.tool_name, "odoo_apply_proposal");
        assert_eq!(row.status, ApprovalStatus::Pending);
        assert_eq!(row.trace_id.as_deref(), Some(id.to_string().as_str()));

        // A second worker racing the same document loses the CAS.
        assert!(!store.set_proposed(id, &proposal(), &call_id).await.expect("cas"));

        let settleable = store.list_settleable(10).await.expect("settleable");
        assert!(settleable.is_empty(), "nothing is settleable while the row is pending");
    });
}

#[tokio::test]
async fn a_denial_from_the_admin_page_lands_on_the_document_when_settled() {
    with_db!(db, {
        let (store, repo) = install(&db).await;
        let owner = UserId::new("admin");
        let id = seed_categorized(&db, "No thanks").await;
        let call_id = propose(&store, &db, &owner, id).await;

        repo.resolve(
            &call_id,
            &ApprovalVerdict {
                status: ApprovalStatus::Denied,
                approver_id: &UserId::new("approver-1"),
                approver_username: "ed",
                note: None,
            },
        )
        .await
        .expect("resolve");

        let due = store.list_settleable(10).await.expect("settleable");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].document_id, id);

        let request = repo.find(&call_id).await.expect("find").expect("row");
        let outcome = settle_document(&store, id, &request, &[]).await.expect("settle");
        assert_eq!(outcome, SettleOutcome::Denied);
        let (status, _, decided_by) = status_of(&db, id).await;
        assert_eq!(status, "denied");
        assert_eq!(decided_by.as_deref(), Some("approver-1"));

        // Settling twice is a no-op, not a second decision.
        let again = settle_document(&store, id, &request, &[]).await.expect("settle again");
        assert_eq!(again, SettleOutcome::NotPending(DocumentStatus::Denied));
    });
}

#[tokio::test]
async fn an_approver_without_an_odoo_link_leaves_the_document_retryable() {
    with_db!(db, {
        let (store, repo) = install(&db).await;
        let owner = UserId::new("admin");
        let id = seed_categorized(&db, "Quote please").await;
        let call_id = propose(&store, &db, &owner, id).await;

        repo.resolve(
            &call_id,
            &ApprovalVerdict {
                status: ApprovalStatus::Approved,
                approver_id: &UserId::new("approver-2"),
                approver_username: "sam",
                note: Some("go"),
            },
        )
        .await
        .expect("resolve");
        let request = repo.find(&call_id).await.expect("find").expect("row");

        let outcome = settle_document(&store, id, &request, &[]).await.expect("settle");
        let SettleOutcome::Failed(error) = outcome else {
            panic!("an unlinked approver cannot write to Odoo: {outcome:?}");
        };
        assert!(error.contains("sam"), "the error names the approver: {error}");
        assert!(error.contains("/admin/profile"), "and the remedy: {error}");

        let (status, stored_error, decided_by) = status_of(&db, id).await;
        assert_eq!(status, "failed");
        assert_eq!(stored_error.as_deref(), Some(error.as_str()));
        assert_eq!(decided_by.as_deref(), Some("approver-2"));

        // The retry is scheduled with backoff, not immediately.
        assert!(store.list_retry_due(10).await.expect("retry due").is_empty());
        let pool = db.pool.pool().expect("pool");
        let attempts: i32 =
            sqlx::query_scalar("SELECT apply_attempts FROM knowledge_documents WHERE id = $1")
                .bind(id)
                .fetch_one(pool.as_ref())
                .await
                .expect("attempts");
        assert_eq!(attempts, 1);
    });
}

#[tokio::test]
async fn the_feed_reads_every_state_back_typed() {
    with_db!(db, {
        let (store, _repo) = install(&db).await;
        let owner = UserId::new("admin");
        let proposed = seed_categorized(&db, "Proposed one").await;
        propose(&store, &db, &owner, proposed).await;
        let skipped = seed_categorized(&db, "Newsletter").await;
        assert!(store.set_skipped(skipped, "noise_category").await.expect("skip"));

        let rows = store
            .list_feed(&Default::default())
            .await
            .expect("feed");
        assert_eq!(rows.len(), 2);
        let by_title = |t: &str| rows.iter().find(|r| r.title == t).expect("row");
        assert_eq!(by_title("Proposed one").status, DocumentStatus::Proposed);
        assert_eq!(
            by_title("Proposed one").proposal.as_ref().map(|p| p.actions.len()),
            Some(2)
        );
        assert_eq!(by_title("Newsletter").status, DocumentStatus::Skipped);
        assert_eq!(by_title("Newsletter").skip_reason.as_deref(), Some("noise_category"));
        assert_eq!(by_title("Newsletter").rfc5322_id(), "<m1@acme.example>");
    });
}
