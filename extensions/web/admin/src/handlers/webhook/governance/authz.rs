//! `POST /govern/authz` — extension webhook implementing
//! [`systemprompt_security::authz::AuthzDecisionHook`] as an HTTP endpoint.
//!
//! Core's gateway and MCP enforcement sites POST an [`AuthzRequest`] here;
//! this handler loads the matching rules from `access_control_rules`, resolves
//! them through the entity's plugin and marketplace parent chain (the same
//! chain the bridge manifest is filtered with), audits the decision to
//! `governance_decisions`, and returns an [`AuthzDecision`] for core to act
//! on. The audit row's `policy` is `authz` regardless of `entity_type`, so
//! `infra logs audit` can correlate gateway and MCP decisions in one stream.
//!
//! The resolver runs over core's `user` / `role` dimensions plus every subject
//! dimension this extension declares in [`crate::authz`] — today that means a
//! `department` rule binds here, not just in the access matrix.

use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;
use systemprompt::identifiers::{ClientId, SessionId};
use systemprompt::loader::ConfigLoader;
use systemprompt_security::authz::{
    AccessControlRepository, AccessRule, AuthzDecision, AuthzRequest, ChainSources, Decision,
    DecisionTag, DenyReason, EntityRow, ParentChainIndex, ResolveBase,
};
use tokio::sync::RwLock;

use crate::authz::{dimensions, subject_attributes_for};
use systemprompt_security::authz::{GovernanceDecisionRecord, insert_governance_decision};

const POLICY_NAME: &str = "authz";

struct CachedChainIndex {
    index: Arc<ParentChainIndex>,
    fetched_at: Instant,
}

static CHAIN_INDEX_CACHE: LazyLock<RwLock<Option<CachedChainIndex>>> =
    LazyLock::new(|| RwLock::new(None));
const CHAIN_INDEX_TTL: Duration = Duration::from_mins(5);

// Why: the same `entity → plugin → marketplace` chain the bridge manifest is
// filtered with (core `keep_sets`), so a decision here and the listing agree.
// Cached for the TTL because every governed tool call lands here; a rule edit
// binds within five minutes, which matches the marketplace cache above it.
async fn chain_index(repo: &AccessControlRepository) -> Arc<ParentChainIndex> {
    {
        let cache = CHAIN_INDEX_CACHE.read().await;
        if let Some(ref cached) = *cache
            && cached.fetched_at.elapsed() < CHAIN_INDEX_TTL
        {
            return Arc::clone(&cached.index);
        }
    }

    let index = match ConfigLoader::load() {
        Ok(services) => ParentChainIndex::load(repo, Arc::new(ChainSources::from_services(&services)))
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "chain_index: parent load failed; resolving without cascade");
                ParentChainIndex::default()
            }),
        Err(e) => {
            tracing::warn!(error = %e, "chain_index: services config unavailable; resolving without cascade");
            ParentChainIndex::default()
        },
    };
    let index = Arc::new(index);

    {
        let mut cache = CHAIN_INDEX_CACHE.write().await;
        *cache = Some(CachedChainIndex {
            index: Arc::clone(&index),
            fetched_at: Instant::now(),
        });
    }
    index
}

async fn load_rules(
    repo: &AccessControlRepository,
    req: &AuthzRequest,
) -> Result<(Vec<AccessRule>, Option<EntityRow>), Response> {
    let kind = req.entity.kind();
    let id = req.entity.id_str();
    let rules = repo
        .list_rules_for_entity(kind, id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, entity_type = %kind, entity_id = %id, "list_rules_for_entity failed");
            // Why: the authz hook's own wire contract — core reads a non-decision
            // status as "hook unavailable", so this must stay distinguishable from
            // a deny body rather than become one. lint-ok: http-error
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "authz hook unavailable: could not load access rules for the entity; see the admin logs for the database error",
            )
                .into_response()
        })?;
    let entity = repo.get_entity(kind, id).await.map_err(|e| {
        tracing::error!(error = %e, entity_type = %kind, entity_id = %id, "get_entity failed");
        // Why: lint-ok: http-error — same wire contract as above.
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "authz hook unavailable: could not load the entity row; see the admin logs for the database error",
        )
            .into_response()
    })?;
    Ok((rules, entity))
}

async fn audit_decision(
    pool: &PgPool,
    req: &AuthzRequest,
    rules: &[AccessRule],
    entity: Option<&EntityRow>,
    decision: &Decision,
) {
    let (decision_tag, reason_str, justification_opt): (DecisionTag, String, Option<String>) =
        match decision {
            Decision::Allow { .. } => (DecisionTag::Allow, String::new(), None),
            Decision::Deny { reason } => (DecisionTag::Deny, reason.to_string(), None),
            // Why: this audits the entity-resolver plane, which has no way to
            // hold a request. Recording the hold verbatim keeps the audit
            // honest about what the chain actually said; the caller decides
            // what to do with it.
            Decision::Pending { reason } => (DecisionTag::Pending, reason.to_string(), None),
        };
    let id = uuid::Uuid::new_v4().to_string();
    let entity_type_str = req.entity.kind().as_str();
    let entity_id_str = req.entity.id_str();
    // JSON: variable-shape: governance audit `evaluated_rules` JSONB payload
    // embedding caller-supplied roles/attributes/context maps, not a
    // template/response body
    let evaluated = serde_json::json!({
        "entity_type": entity_type_str,
        "entity_id": entity_id_str,
        "trace_id": req.trace_id.as_str(),
        "roles": req.roles,
        "attributes": req.attributes,
        "context": req.context,
        "actor": req.actor(),
        "client_id": req.client_id,
        "entity": entity,
        "justification": justification_opt,
        "rules": rules,
    });
    let actor = req.actor();
    // Why: enforcement sites without an explicit context still need one the
    // session's other rows join to; deriving keeps them in a single context.
    let context_id = req.context_id.clone().unwrap_or_else(|| {
        req.session_id.as_ref().map_or_else(
            systemprompt::identifiers::ContextId::legacy,
            systemprompt::identifiers::ContextId::derived_from_session,
        )
    });
    let record = GovernanceDecisionRecord {
        id: &id,
        actor: &actor,
        // Why: the attested session, so a gateway decision keys to the same
        // session row as the prompt gate and the `ai_requests` row it belongs
        // to. Enforcement sites without a session (server-attach RBAC, MCP)
        // send none, and the trace join reads `trace_id` rather than this
        // field.
        session_id: req.session_id.as_ref().map_or("", SessionId::as_str),
        tool_name: entity_id_str,
        agent_id: req.verified_agent_id(),
        agent_scope: req.access_scope,
        decision: decision_tag,
        policy: POLICY_NAME,
        reason: &reason_str,
        evaluated_rules: &evaluated,
        plugin_id: None,
        act_chain: &req.act_chain,
        context_id: context_id.as_str(),
        task_id: req
            .task_id
            .as_ref()
            .map(systemprompt::identifiers::TaskId::as_str),
        trace_id: Some(req.trace_id.as_str()),
        client_id: req.client_id.as_ref().map(ClientId::as_str),
    };
    if let Err(e) = insert_governance_decision(pool, &record).await {
        tracing::error!(error = %e, "Failed to record authz decision");
    }
}

pub(crate) async fn govern_authz(
    State(pool): State<Arc<PgPool>>,
    Json(req): Json<AuthzRequest>,
) -> Response {
    // Why: lint-ok: http-error — a hook answers 200 with a decision; an error
    // status reads as "hook unavailable" and lets the call through
    let repo = AccessControlRepository::from_pool(Arc::clone(&pool));

    let (rules, entity) = match load_rules(&repo, &req).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let index = chain_index(&repo).await;

    // Why: resolved by lookup rather than read off the request, so a department
    // change or a revocation binds on the next call instead of waiting for the
    // caller's token to refresh.
    let attributes = subject_attributes_for(&pool, &req.user_id).await;

    let decision = index.resolve(
        req.entity.kind(),
        req.entity.id_str(),
        ResolveBase {
            rules: &rules,
            user_id: &req.user_id,
            user_roles: &req.roles,
            default_included: entity.as_ref().map(|e| e.default_included),
            attributes: &attributes,
            dimensions: dimensions(&pool),
        },
    );

    audit_decision(&pool, &req, &rules, entity.as_ref(), &decision).await;

    let resp = match decision {
        Decision::Allow { .. } => AuthzDecision::Allow,
        Decision::Deny { reason } => AuthzDecision::Deny {
            reason,
            policy: POLICY_NAME.to_owned(),
        },
        // Why: `AuthzDecision` is a two-valued wire type — this endpoint
        // answers "may this subject reach this entity", which the caller
        // cannot park. A hold arriving here means a holding policy was mounted
        // on a plane that cannot honour it, so it degrades to a deny. Failing
        // the other way would turn the strictest verdict into an allow.
        Decision::Pending { reason } => {
            tracing::error!(
                %reason,
                "a governance hold reached the authz webhook, which cannot park a request; \
                 refusing it"
            );
            AuthzDecision::Deny {
                reason: DenyReason::PolicyViolation {
                    policy: "require_approval".to_owned(),
                    detail: std::borrow::Cow::Borrowed(
                        "approval required, but this enforcement point cannot hold a request",
                    ),
                },
                policy: POLICY_NAME.to_owned(),
            }
        },
    };
    (StatusCode::OK, Json(resp)).into_response()
}
