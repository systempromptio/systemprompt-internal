//! Resolving what a caller actually holds, for `whoami`.
//!
//! Why this is beside the handler rather than in it: the handler's job is to
//! answer one tool call, and everything here is the access-control read that
//! answer is built from — the id sets, the keep-filter per entity kind, and
//! the rules written directly against an entity. Kept apart, a change to what
//! is granted cannot be mistaken for a change to how it is reported.

use std::collections::{BTreeSet, HashMap, HashSet};

use systemprompt::identifiers::PluginId;
use systemprompt::models::services::ServicesConfig;
use systemprompt::security::authz::{
    AccessControlRepository, AccessRule, BulkKeepQuery, ChainSources, EntityKind,
    NO_SUBJECT_ATTRIBUTES, ParentChainIndex, allowed_ids,
};

use crate::store::IdentityRow;
use crate::whoami::{GrantSource, GrantedEntity, WhoamiGrants};

// Why: the id sets the grant resolution runs over, gathered in one place so
// `resolve_grants` reads as resolution rather than as collection. Only enabled
// MCP servers are listed: a disabled server is not a grant a caller can hold.
pub(super) struct CatalogIds {
    marketplace_id: Option<String>,
    plugin_ids: Vec<String>,
    mcp_ids: Vec<String>,
    skill_ids: Vec<String>,
}

pub(super) fn catalog_ids(services: &ServicesConfig, sources: &ChainSources) -> CatalogIds {
    CatalogIds {
        marketplace_id: sources
            .marketplace
            .as_ref()
            .map(|m| m.id.as_str().to_owned()),
        plugin_ids: sources
            .plugins
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        mcp_ids: services
            .mcp_servers
            .iter()
            .filter(|(_, d)| d.enabled)
            .map(|(name, _)| name.clone())
            .collect(),
        skill_ids: sources
            .skill_owners
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect(),
    }
}

// Why: rules written directly against the entity, as opposed to a grant it
// inherits from its plugin or the marketplace. The distinction is what
// `GrantSource::Own` reports.
struct OwnRules {
    plugins: HashMap<String, Vec<AccessRule>>,
    mcp_servers: HashMap<String, Vec<AccessRule>>,
    skills: HashMap<String, Vec<AccessRule>>,
}

async fn own_rules(
    repo: &AccessControlRepository,
    plugin_ids: &[String],
    mcp_ids: &[String],
    skill_ids: &[String],
) -> Result<OwnRules, systemprompt::security::authz::AuthzError> {
    Ok(OwnRules {
        plugins: repo.list_rules_bulk(EntityKind::Plugin, plugin_ids).await?,
        mcp_servers: repo
            .list_rules_bulk(EntityKind::McpServer, mcp_ids)
            .await?,
        skills: repo.list_rules_bulk(EntityKind::Skill, skill_ids).await?,
    })
}

// Why: which ids survive the keep-filter for this caller, per kind. Split out
// so `resolve_grants` states what it is resolving rather than how each kind is
// queried; the borrow dance exists because every call shares one repo and one
// loaded chain index.
struct AllowedSets {
    marketplaces: HashSet<String>,
    plugins: HashSet<String>,
    mcp_servers: HashSet<String>,
    skills: HashSet<String>,
}

async fn allowed_sets(
    repo: &AccessControlRepository,
    index: &ParentChainIndex,
    identity: &IdentityRow,
    ids: &CatalogIds,
) -> Result<AllowedSets, systemprompt::security::authz::AuthzError> {
    let allowed = |kind: EntityKind, ids: &[String]| {
        let ids = ids.to_vec();
        async move {
            allowed_ids(
                repo,
                BulkKeepQuery {
                    user_id: &identity.id,
                    roles: &identity.roles,
                    kind,
                    ids: &ids,
                    chains: index,
                    attributes: &NO_SUBJECT_ATTRIBUTES,
                    dimensions: &[],
                },
            )
            .await
        }
    };

    let marketplace_ids: Vec<String> = ids.marketplace_id.iter().cloned().collect();
    Ok(AllowedSets {
        marketplaces: allowed(EntityKind::Marketplace, &marketplace_ids).await?,
        plugins: allowed(EntityKind::Plugin, &ids.plugin_ids).await?,
        mcp_servers: allowed(EntityKind::McpServer, &ids.mcp_ids).await?,
        skills: allowed(EntityKind::Skill, &ids.skill_ids).await?,
    })
}

// Why: a rule written directly against the entity is what makes a grant the
// caller's own rather than one inherited from a parent.
fn has_own(rules: &HashMap<String, Vec<AccessRule>>, id: &str) -> bool {
    rules.get(id).is_some_and(|r| !r.is_empty())
}

fn granted(
    ids: &[String],
    allowed: &HashSet<String>,
    via: &dyn Fn(&str) -> GrantSource,
) -> Vec<GrantedEntity> {
    ids.iter()
        .filter(|id| allowed.contains(*id))
        .map(|id| GrantedEntity {
            id: id.clone(),
            via: via(id),
        })
        .collect()
}

pub(super) async fn resolve_grants(
    db: &systemprompt::database::DbPool,
    services: &ServicesConfig,
    identity: &IdentityRow,
) -> Result<WhoamiGrants, systemprompt::security::authz::AuthzError> {
    let repo = AccessControlRepository::new(db)?;
    let sources = ChainSources::from_services(services);
    let index = ParentChainIndex::load(&repo, sources.clone()).await?;

    let CatalogIds {
        marketplace_id,
        plugin_ids,
        mcp_ids,
        skill_ids,
    } = catalog_ids(services, &sources);

    let AllowedSets {
        marketplaces,
        plugins,
        mcp_servers,
        skills,
    } = allowed_sets(
        &repo,
        &index,
        identity,
        &CatalogIds {
            marketplace_id: marketplace_id.clone(),
            plugin_ids: plugin_ids.clone(),
            mcp_ids: mcp_ids.clone(),
            skill_ids: skill_ids.clone(),
        },
    )
    .await?;

    let OwnRules {
        plugins: own_plugins,
        mcp_servers: own_mcp,
        skills: own_skills,
    } = own_rules(&repo, &plugin_ids, &mcp_ids, &skill_ids).await?;
    let via_marketplace = || {
        marketplace_id
            .clone()
            .map_or(GrantSource::Own, GrantSource::Marketplace)
    };
    let marketplace_ids: Vec<String> = marketplace_id.iter().cloned().collect();

    Ok(WhoamiGrants {
        marketplaces: granted(&marketplace_ids, &marketplaces, &|_| GrantSource::Own),
        plugins: granted(&plugin_ids, &plugins, &|id| {
            if has_own(&own_plugins, id) {
                GrantSource::Own
            } else {
                via_marketplace()
            }
        }),
        mcp_servers: granted(&mcp_ids, &mcp_servers, &|id| {
            if has_own(&own_mcp, id) {
                GrantSource::Own
            } else {
                via_marketplace()
            }
        }),
        skills: granted(&skill_ids, &skills, &|id| {
            if has_own(&own_skills, id) {
                return GrantSource::Own;
            }
            // Why: `SkillId` and `PluginId` impl `Borrow<str>`, so the map
            // still takes the plain `&str` that `granted` hands this closure —
            // only the value type changed when core made these ids typed.
            let owners: &BTreeSet<PluginId> = match sources.skill_owners.get(id) {
                Some(owners) => owners,
                None => return via_marketplace(),
            };
            owners
                .iter()
                .find(|owner| plugins.contains(owner.as_str()))
                .map(|owner| owner.as_str().to_owned())
                .map_or_else(via_marketplace, GrantSource::Plugin)
        }),
    })
}
