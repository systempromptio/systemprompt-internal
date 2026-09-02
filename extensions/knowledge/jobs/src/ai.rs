//! Building the AI gateway client a job needs, from the same services config
//! the server uses — so a job's inference lands in the same
//! audit spine, with the same provider defaults, as an interactive request.

use std::sync::Arc;

use systemprompt::ai::{AiService, AiServiceProviders};
use systemprompt::analytics::AnalyticsAiSessionProvider;
use systemprompt::database::DbPool;
use systemprompt::loader::ConfigLoader;
use systemprompt::mcp::McpToolProvider;
use systemprompt::system::AppContext;

use crate::error::KnowledgeJobError;

pub(crate) fn build_ai_service(
    db_pool: &DbPool,
    app_context: &Arc<AppContext>,
) -> Result<Arc<AiService>, KnowledgeJobError> {
    let services_config = ConfigLoader::load().map_err(other)?;

    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(db_pool),
        app_context.mcp_registry().clone(),
        &services_config.ai.mcp.resilience,
    ));
    let session_provider = Arc::new(AnalyticsAiSessionProvider::from_repository(
        app_context.analytics_repositories().sessions.clone(),
    ));
    Ok(Arc::new(
        AiService::new(
            db_pool,
            &services_config.providers,
            &services_config.ai,
            AiServiceProviders {
                tools: tool_provider,
                sessions: session_provider,
            },
            app_context.ai_repositories(),
        )
        .map_err(other)?
        .with_context_materializer(app_context.context_materializer()),
    ))
}

fn other(e: impl std::fmt::Display) -> KnowledgeJobError {
    KnowledgeJobError::Other(e.to_string())
}
