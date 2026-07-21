//! Activity constructors for sign-in and agent-response events.

use serde::Serialize;
use systemprompt::identifiers::{SessionId, UserId};

use super::constructors::truncate;
use super::enums::{ActivityAction, ActivityCategory, ActivityEntity};
use super::types::{ActivityEntityRef, NewActivity};

/// Metadata payload for events that carry no fields of their own.
fn empty_meta() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// Shared shape for events that only carry the session id.
#[derive(Debug, Serialize)]
struct SessionMeta<'a> {
    session_id: &'a str,
}

#[derive(Debug, Serialize)]
struct SessionStartedMeta<'a> {
    session_id: &'a str,
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<&'a str>,
}

impl NewActivity {
    #[must_use]
    pub fn login(user_id: &UserId, display_name: &str) -> Self {
        Self {
            user_id: user_id.clone(),
            category: ActivityCategory::Login,
            action: ActivityAction::LoggedIn,
            entity: None,
            description: format!("{display_name} logged in"),
            metadata: empty_meta(),
        }
    }

    #[must_use]
    pub fn session_started(
        user_id: &UserId,
        session_id: &SessionId,
        model: &str,
        project_path: Option<&str>,
    ) -> Self {
        let meta = serde_json::to_value(SessionStartedMeta {
            session_id: session_id.as_str(),
            model,
            project_path,
        })
        .unwrap_or_default();
        Self {
            user_id: user_id.clone(),
            category: ActivityCategory::Session,
            action: ActivityAction::Started,
            entity: Some(ActivityEntityRef {
                kind: ActivityEntity::Session,
                id: Some(session_id.as_str().to_owned()),
                name: None,
            }),
            description: format!("Started a session ({model})"),
            metadata: meta,
        }
    }

    #[must_use]
    pub fn session_ended(user_id: &UserId, session_id: &SessionId) -> Self {
        Self {
            user_id: user_id.clone(),
            category: ActivityCategory::Session,
            action: ActivityAction::Ended,
            entity: Some(ActivityEntityRef {
                kind: ActivityEntity::Session,
                id: Some(session_id.as_str().to_owned()),
                name: None,
            }),
            description: "Ended a session".to_owned(),
            metadata: serde_json::to_value(SessionMeta {
                session_id: session_id.as_str(),
            })
            .unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn agent_response(
        user_id: &UserId,
        session_id: &SessionId,
        message_preview: Option<&str>,
    ) -> Self {
        let description = message_preview.map_or_else(
            || "Claude finished responding".to_owned(),
            |msg| format!("Claude responded: \"{}\"", truncate(msg, 80)),
        );
        Self {
            user_id: user_id.clone(),
            category: ActivityCategory::AgentResponse,
            action: ActivityAction::Submitted,
            entity: None,
            description,
            metadata: serde_json::to_value(SessionMeta {
                session_id: session_id.as_str(),
            })
            .unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn teammate_idle(user_id: &UserId, session_id: &SessionId, name: Option<&str>) -> Self {
        let who = name.unwrap_or("unknown");
        Self {
            user_id: user_id.clone(),
            category: ActivityCategory::Session,
            action: ActivityAction::Ended,
            entity: Some(ActivityEntityRef {
                kind: ActivityEntity::Agent,
                id: Some(session_id.as_str().to_owned()),
                name: name.map(str::to_owned),
            }),
            description: format!("Teammate {who} went idle"),
            metadata: serde_json::to_value(SessionMeta {
                session_id: session_id.as_str(),
            })
            .unwrap_or_default(),
        }
    }
}
