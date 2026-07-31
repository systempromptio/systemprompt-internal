//! Salesforce org configuration as code.
//!
//! `services/salesforce/org.yaml` declares what an org should look like; this
//! module reads an org into that shape, compares the two, and makes an org
//! match. Standing up a second org becomes a reviewable diff plus a command
//! rather than a click-path through Setup.
//!
//! # What Salesforce actually allows
//!
//! Verified against a live org rather than assumed, because the API surface is
//! uneven and the uneven parts drive the design:
//!
//! | Operation | Path |
//! |---|---|
//! | Read the app, its OAuth settings and policies | REST / Tooling SOQL |
//! | Write them | Metadata API only — all four sObjects are `createable: false` |
//! | Permission sets, app grants, assignments | REST sObject writes |
//! | Activate a standard hosted MCP server | no API; Setup only |
//!
//! The SOAP Metadata API rejects this deployment's tokens outright ("SOAP API
//! does not support JWT-based access tokens"), which rules out the `sf` CLI.
//! The Metadata *REST* deploy resource accepts them, so [`client`] deploys over
//! REST and the whole loop stays headless with the credentials the platform
//! already holds.
//!
//! # What cannot round-trip
//!
//! `callback_url`, `pkce_required` and `consumer_secret_optional` live on
//! `ExtlClntAppGlobalOauthSettings`, which is not a queryable sObject. They are
//! deployed on every apply and reported as `always-applied` by [`diff`] rather
//! than being counted as verified.

pub mod apply;
pub mod client;
pub mod deploy;
pub mod diff;
pub mod export;
pub mod scope;
pub mod spec;

pub use client::{Connection, TargetOrg};
pub use deploy::DeployResult;
pub use diff::{Change, ChangeKind, ChangeSet};
pub use scope::OauthScope;
pub use spec::{OrgSpec, SPEC_RELATIVE_PATH, SpecError};
