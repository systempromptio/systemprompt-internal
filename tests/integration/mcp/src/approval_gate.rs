//! The approval rendezvous against live Postgres.
//!
//! Unit tests cover the `require_approval` verdict; what they cannot cover is
//! the part that actually makes the feature work — one task blocking on a row
//! while a different one resolves it, which is how the MCP server and the
//! admin console meet. These tests drive that directly.

use std::time::Duration;

use systemprompt::identifiers::{CallId, UserId};
use systemprompt::security::policy::{
    ApprovalOutcome, ApprovalRepository, ApprovalStatus, ApprovalVerdict, NewApprovalRequest,
    wait_for_decision,
};

use super::common::TempDb;

// Mirrors crates/infra/security/schema/approval_requests.sql. Declared here
// rather than shared with common.rs so the other suites in this crate keep
// their minimal two-table schema.
const SCHEMA: &str = r"CREATE TABLE approval_requests (
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
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT approval_decided_fields CHECK (
        (status = 'pending' AND approver_id IS NULL AND decided_at IS NULL)
        OR (status <> 'pending')
    )
)";

async fn repo(db: &TempDb) -> ApprovalRepository {
    let pool = db.pool.pool().expect("throwaway pool");
    sqlx::query(SCHEMA)
        .execute(pool.as_ref())
        .await
        .expect("create approval_requests");
    ApprovalRepository::new((*pool).clone())
}

// Owns the values `NewApprovalRequest` borrows, so a held call can be built
// without returning references into a temporary.
struct Fixture {
    requester: UserId,
    arguments: serde_json::Value,
}

impl Fixture {
    fn new() -> Self {
        Self {
            requester: UserId::new("e2e-sales"),
            arguments: serde_json::json!({"lead_id": 7, "body": "hi"}),
        }
    }

    fn held<'a>(&'a self, call_id: &'a CallId, expires_in_seconds: u64) -> NewApprovalRequest<'a> {
        NewApprovalRequest {
            call_id,
            tool_name: "note_add",
            server_name: "odoo",
            arguments: &self.arguments,
            requested_by: &self.requester,
            session_id: None,
            trace_id: Some("trace-1"),
            rule: "note_add",
            expires_in_seconds,
        }
    }
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
async fn a_waiting_call_observes_an_approval_made_by_another_task() {
    with_db!(db, {
        let repo = repo(&db).await;
        let fx = Fixture::new();
        let call = CallId::new("call-approved");
        repo.open(&fx.held(&call, 900)).await.expect("open");

        // The whole point of the feature: the gate is blocked here while a
        // different task — the admin console, in production — resolves it.
        let approver = {
            let repo = repo.clone();
            let call = call.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                repo.resolve(
                    call.as_str(),
                    &ApprovalVerdict {
                        status: ApprovalStatus::Approved,
                        approver_id: &UserId::new("admin-1"),
                        approver_username: "ed",
                        note: Some("looks right"),
                    },
                )
                .await
                .expect("resolve")
            })
        };

        let outcome = wait_for_decision(&repo, call.as_str(), Duration::from_secs(10)).await;
        approver
            .await
            .expect("approver task")
            .expect("resolved row");

        match outcome {
            ApprovalOutcome::Approved(request) => {
                assert_eq!(request.approver_username.as_deref(), Some("ed"));
                assert_eq!(request.decision_note.as_deref(), Some("looks right"));
                assert!(request.decided_at.is_some());
            },
            other => panic!("expected Approved, got {other:?}"),
        }
    });
}

#[tokio::test]
async fn a_refusal_comes_back_as_denied_with_the_refuser() {
    with_db!(db, {
        let repo = repo(&db).await;
        let fx = Fixture::new();
        let call = CallId::new("call-denied");
        repo.open(&fx.held(&call, 900)).await.expect("open");

        repo.resolve(
            call.as_str(),
            &ApprovalVerdict {
                status: ApprovalStatus::Denied,
                approver_id: &UserId::new("admin-1"),
                approver_username: "ed",
                note: None,
            },
        )
        .await
        .expect("resolve")
        .expect("row was still pending");

        let outcome = wait_for_decision(&repo, call.as_str(), Duration::from_secs(5)).await;
        match outcome {
            ApprovalOutcome::Denied(request) => {
                assert_eq!(request.approver_username.as_deref(), Some("ed"));
            },
            other => panic!("expected Denied, got {other:?}"),
        }
    });
}

#[tokio::test]
async fn an_unanswered_round_hands_the_wait_back_instead_of_blocking_forever() {
    with_db!(db, {
        let repo = repo(&db).await;
        let fx = Fixture::new();
        let call = CallId::new("call-still-pending");
        repo.open(&fx.held(&call, 900)).await.expect("open");

        // Short hold, long expiry: the round gives up but the approval is
        // still open, which is what becomes an MRTR retry.
        let outcome = wait_for_decision(&repo, call.as_str(), Duration::from_millis(600)).await;
        assert!(
            matches!(outcome, ApprovalOutcome::StillPending(_)),
            "expected StillPending, got {outcome:?}"
        );

        // And the approval genuinely survives the round.
        let found = repo.find(call.as_str()).await.expect("find").expect("row");
        assert_eq!(found.status, ApprovalStatus::Pending);
    });
}

#[tokio::test]
async fn a_retry_rejoins_the_same_approval_rather_than_opening_a_second() {
    with_db!(db, {
        let repo = repo(&db).await;
        let fx = Fixture::new();
        let call = CallId::new("call-retried");
        let first = repo.open(&fx.held(&call, 60)).await.expect("first open");

        // A later round asks for a much longer expiry. If `open` upserted, the
        // deadline would move every retry and the call could never expire.
        let second = repo
            .open(&fx.held(&call, 9_000))
            .await
            .expect("second open");

        assert_eq!(first.call_id, second.call_id);
        assert_eq!(
            first.expires_at, second.expires_at,
            "a retry must not extend the deadline"
        );
        assert_eq!(repo.list_pending(10).await.expect("list").len(), 1);
    });
}

#[tokio::test]
async fn a_decision_already_taken_cannot_be_overwritten() {
    with_db!(db, {
        let repo = repo(&db).await;
        let fx = Fixture::new();
        let call = CallId::new("call-raced");
        repo.open(&fx.held(&call, 900)).await.expect("open");

        let admin = UserId::new("admin-x");
        let verdict = |status, who| ApprovalVerdict {
            status,
            approver_id: &admin,
            approver_username: who,
            note: None,
        };

        let first = repo
            .resolve(call.as_str(), &verdict(ApprovalStatus::Approved, "ed"))
            .await
            .expect("first resolve");
        assert!(first.is_some(), "first decision should land");

        // A second admin clicking Deny on the same queue entry.
        let second = repo
            .resolve(
                call.as_str(),
                &verdict(ApprovalStatus::Denied, "someone-else"),
            )
            .await
            .expect("second resolve");
        assert!(second.is_none(), "a resolved call must not be re-decided");

        let found = repo.find(call.as_str()).await.expect("find").expect("row");
        assert_eq!(found.status, ApprovalStatus::Approved);
        assert_eq!(found.approver_username.as_deref(), Some("ed"));
    });
}

#[tokio::test]
async fn an_expired_call_is_swept_and_cannot_be_approved_late() {
    with_db!(db, {
        let repo = repo(&db).await;
        let fx = Fixture::new();
        let call = CallId::new("call-expired");
        // Already past its deadline the moment it is opened.
        repo.open(&fx.held(&call, 0)).await.expect("open");

        let outcome = wait_for_decision(&repo, call.as_str(), Duration::from_millis(200)).await;
        assert!(
            matches!(outcome, ApprovalOutcome::Expired(_)),
            "expected Expired, got {outcome:?}"
        );

        assert_eq!(repo.expire_due().await.expect("sweep"), 1);

        let late = repo
            .resolve(
                call.as_str(),
                &ApprovalVerdict {
                    status: ApprovalStatus::Approved,
                    approver_id: &UserId::new("admin-1"),
                    approver_username: "ed",
                    note: None,
                },
            )
            .await
            .expect("late resolve");
        assert!(late.is_none(), "an abandoned call must not be revivable");

        // And it never appears in the console queue.
        assert!(repo.list_pending(10).await.expect("list").is_empty());
    });
}
