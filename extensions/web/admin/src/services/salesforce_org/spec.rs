//! The desired-state document: `services/salesforce/org.yaml`.
//!
//! This is the source of truth for what a Salesforce org should look like.
//! [`export`](super::export) produces one from a live org,
//! [`diff`](super::diff) compares two, and [`apply`](super::apply) makes an org
//! match one.
//!
//! Record ids, consumer keys and org ids are deliberately absent: they are
//! per-org and minted by Salesforce, so a spec carrying them could not be
//! applied to a second org.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::scope::OauthScope;

/// Where the spec lives relative to the `services/` root.
pub const SPEC_RELATIVE_PATH: &str = "salesforce/org.yaml";

#[derive(Debug, Clone, thiserror::Error)]
pub enum SpecError {
    #[error("salesforce org spec not found at {0}")]
    NotFound(PathBuf),
    #[error("failed to read {path}: {message}")]
    Read { path: PathBuf, message: String },
    #[error("failed to parse {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("failed to serialise org spec: {0}")]
    Serialise(String),
}

/// The whole desired state of an org.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrgSpec {
    pub external_client_app: ExternalClientApp,
    #[serde(default)]
    pub permission_sets: Vec<PermissionSetSpec>,
    /// Standard hosted MCP servers. These are *asserted*, never created — no
    /// API to activate them was found, so apply reports an inactive one as an
    /// actionable error rather than pretending to fix it.
    #[serde(default)]
    pub hosted_mcp_servers: Vec<HostedMcpServer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalClientApp {
    /// Immutable API name. Every dependent metadata record keys off this, so
    /// renaming it creates a second app rather than renaming the first.
    pub developer_name: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub contact_email: String,
    /// Salesforce enum `ExtlClntAppDistState`. Salesforce does not publish the
    /// value set and its parse error does not enumerate it; `Local` is verified
    /// against a live org. Invalid values fail the deploy with a clear message.
    pub distribution_state: String,
    pub oauth: OauthSpec,
    pub policies: PolicySpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OauthSpec {
    /// Must match the platform's `redirect_uri` in `salesforce.yaml` exactly —
    /// Salesforce compares them character for character.
    pub callback_url: String,
    pub scopes: Vec<OauthScope>,
    #[serde(default)]
    pub first_party_app_enabled: bool,
    #[serde(default = "default_true")]
    pub pkce_required: bool,
    #[serde(default)]
    pub consumer_secret_optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_logout_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySpec {
    /// Salesforce enum `PermittedUsersPolicyType`. `AdminApprovedPreAuthorized`
    /// is verified live and is what gates access to a permission set.
    pub permitted_users: String,
    pub ip_relaxation: IpRelaxation,
    /// Salesforce enum `RefreshTokenPolicyType`; `SpecificLifetime` verified.
    pub refresh_token_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token_validity: Option<Validity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_session_level: Option<String>,
}

/// Verified exhaustively: Salesforce's validation error names the full set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpRelaxation {
    Enforce,
    Bypass,
    #[serde(rename = "Bypass_2factor")]
    Bypass2Factor,
    #[serde(rename = "Enforce_relaxrefresh")]
    EnforceRelaxRefresh,
}

impl IpRelaxation {
    #[must_use]
    pub const fn metadata_token(self) -> &'static str {
        match self {
            Self::Enforce => "Enforce",
            Self::Bypass => "Bypass",
            Self::Bypass2Factor => "Bypass_2factor",
            Self::EnforceRelaxRefresh => "Enforce_relaxrefresh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validity {
    pub period: u32,
    pub unit: ValidityUnit,
}

/// Verified exhaustively: "Set the refresh token validity unit to Days, Hours,
/// Months."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidityUnit {
    Hours,
    Days,
    Months,
}

impl ValidityUnit {
    #[must_use]
    pub const fn metadata_token(self) -> &'static str {
        match self {
            Self::Hours => "Hours",
            Self::Days => "Days",
            Self::Months => "Months",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionSetSpec {
    pub name: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Developer name of the External Client App this permission set
    /// pre-authorizes. This is the `SetupEntityAccess` grant, and it is what
    /// makes `PermittedUsersPolicyType: AdminApprovedPreAuthorized` usable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grants_app: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedMcpServer {
    pub name: String,
    pub endpoint: String,
}

const fn default_true() -> bool {
    true
}

impl OrgSpec {
    /// Read a spec from disk.
    ///
    /// # Errors
    /// [`SpecError::NotFound`] if the path does not exist, [`SpecError::Read`]
    /// on an unreadable file, [`SpecError::Parse`] on malformed or unknown
    /// YAML.
    pub fn load(path: &Path) -> Result<Self, SpecError> {
        if !path.exists() {
            return Err(SpecError::NotFound(path.to_path_buf()));
        }
        let raw = std::fs::read_to_string(path).map_err(|e| SpecError::Read {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        serde_yaml::from_str(&raw).map_err(|e| SpecError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Render the spec back to YAML.
    ///
    /// # Errors
    /// [`SpecError::Serialise`] if the spec cannot be represented as YAML.
    pub fn to_yaml(&self) -> Result<String, SpecError> {
        serde_yaml::to_string(self).map_err(|e| SpecError::Serialise(e.to_string()))
    }
}
