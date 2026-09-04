//! The KPI strip shared by all four demo pages.

use serde::Serialize;
use sqlx::PgPool;

use super::filter::DemoFilter;

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct DemoKpis {
    pub skill_invocations: i64,
    pub mcp_calls: i64,
    pub mcp_failures: i64,
    pub held: i64,
    pub refused: i64,
    pub blocked: i64,
    pub approved: i64,
    pub attributed_tokens: i64,
    pub attributed_cost_microdollars: i64,
}

pub async fn get_demo_kpis(_pool: &PgPool, _filter: &DemoFilter) -> Result<DemoKpis, sqlx::Error> {
    Ok(DemoKpis::default())
}
