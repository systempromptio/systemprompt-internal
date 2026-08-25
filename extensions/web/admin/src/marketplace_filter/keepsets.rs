//! Pure plumbing for `TemplateMarketplaceFilter`: candidate id extraction and
//! the keep-ids query shape. None of this touches the database — it is the
//! deterministic shape-shuffling around the access-control resolver, split out
//! to keep the filter module focused on the query flow.

use systemprompt::marketplace::MarketplaceCandidate;
use systemprompt_security::authz::{EntityKind, ResolveParent};

#[derive(Debug)]
pub struct CandidateEntityIds {
    pub plugins: Vec<String>,
    pub skills: Vec<String>,
    pub agents: Vec<String>,
    pub hooks: Vec<String>,
    pub mcp: Vec<String>,
}

impl CandidateEntityIds {
    pub fn from_candidate(candidate: &MarketplaceCandidate) -> Self {
        Self {
            plugins: candidate.plugins.iter().map(|p| p.id.to_string()).collect(),
            skills: candidate.skills.iter().map(|s| s.id.to_string()).collect(),
            agents: candidate.agents.iter().map(|a| a.id.to_string()).collect(),
            hooks: candidate.hooks.iter().map(|h| h.id.to_string()).collect(),
            mcp: candidate
                .managed_mcp_servers
                .iter()
                .map(|m| m.name.to_string())
                .collect(),
        }
    }
}

pub type KeepSet = std::collections::HashSet<String>;

#[derive(Debug)]
pub struct KeepIdsQuery<'a> {
    pub user_id: &'a str,
    pub roles: &'a [String],
    pub kind: EntityKind,
    pub ids: &'a [String],
    pub parents: &'a [ResolveParent<'a>],
}
