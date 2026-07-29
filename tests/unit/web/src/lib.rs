//! Unit tests for `systemprompt-web-shared` pure logic:
//! - `CampaignLink::full_url` UTM query assembly and `?`/`&` separator choice
//! - `BlogConfigValidated::validate` base-URL scheme/parse validation
//! - hook-event ingest leniency, which the governance record depends on
//! - admin display formatting bands and `PageWindow` pagination arithmetic
//! - inventory registry completeness for jobs, renderers, and providers
//! - calendar-month resolution and the month-end P&L's derived figures

#[cfg(test)]
mod campaign_link_full_url;
#[cfg(test)]
mod config_base_url;
#[cfg(test)]
mod format_display;
#[cfg(test)]
mod hook_event_dispatch;
#[cfg(test)]
mod month_range;
#[cfg(test)]
mod page_window;
#[cfg(test)]
mod registry_completeness;
#[cfg(test)]
mod report_pnl;
#[cfg(test)]
mod seed_contract;
