use systemprompt::loader::ConfigLoader;
use systemprompt::models::services::MarketplaceConfig;

pub(crate) fn load_marketplaces() -> Vec<MarketplaceConfig> {
    let Ok(services) = ConfigLoader::load() else {
        return Vec::new();
    };

    let mut entries: Vec<MarketplaceConfig> = services.marketplaces.into_values().collect();
    entries.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    entries
}
