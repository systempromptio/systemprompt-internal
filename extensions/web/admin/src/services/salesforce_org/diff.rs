//! Compare a desired [`OrgSpec`] against an org's actual one.
//!
//! A pure function over two specs, so it is testable without an org.
//!
//! Fields that no API can read back are reported as
//! [`ChangeKind::AlwaysApplied`] rather than folded into "no changes". Calling
//! them unchanged would be a claim the tool cannot support: it has not read
//! them and does not know.

use std::fmt;

use super::spec::{OrgSpec, PermissionSetSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Present in both, with different values.
    Update,
    /// In the desired spec, absent from the org.
    Add,
    /// In the org, absent from the desired spec.
    Remove,
    /// Deployed on every apply because it cannot be read back to compare.
    AlwaysApplied,
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Update => "update",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::AlwaysApplied => "always-applied",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    /// Dotted path into the spec, e.g. `external_client_app.oauth.scopes`.
    pub path: String,
    pub actual: String,
    pub desired: String,
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ChangeKind::AlwaysApplied => {
                write!(f, "  {} {} = {}", self.kind, self.path, self.desired)
            },
            _ => write!(
                f,
                "  {} {}: {} -> {}",
                self.kind, self.path, self.actual, self.desired
            ),
        }
    }
}

/// The full result of comparing two specs.
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    pub changes: Vec<Change>,
}

impl ChangeSet {
    /// Changes that represent real, detected drift.
    #[must_use]
    pub fn drift(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|c| c.kind != ChangeKind::AlwaysApplied)
            .collect()
    }

    /// Whether the org matches the spec on everything readable.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.drift().is_empty()
    }
}

fn push(out: &mut Vec<Change>, kind: ChangeKind, path: &str, actual: &str, desired: &str) {
    out.push(Change {
        kind,
        path: path.to_owned(),
        actual: actual.to_owned(),
        desired: desired.to_owned(),
    });
}

fn compare_str(out: &mut Vec<Change>, path: &str, actual: &str, desired: &str) {
    if actual != desired {
        push(out, ChangeKind::Update, path, actual, desired);
    }
}

/// Compare `actual` (as exported from an org) against `desired`.
#[must_use]
pub fn diff(actual: &OrgSpec, desired: &OrgSpec) -> ChangeSet {
    let mut changes = Vec::new();
    let (a, d) = (&actual.external_client_app, &desired.external_client_app);

    compare_str(
        &mut changes,
        "external_client_app.developer_name",
        &a.developer_name,
        &d.developer_name,
    );
    compare_str(
        &mut changes,
        "external_client_app.label",
        &a.label,
        &d.label,
    );
    compare_str(
        &mut changes,
        "external_client_app.description",
        a.description.as_deref().unwrap_or(""),
        d.description.as_deref().unwrap_or(""),
    );
    compare_str(
        &mut changes,
        "external_client_app.contact_email",
        &a.contact_email,
        &d.contact_email,
    );
    compare_str(
        &mut changes,
        "external_client_app.distribution_state",
        &a.distribution_state,
        &d.distribution_state,
    );

    diff_scopes(&mut changes, a, d);
    diff_policies(&mut changes, a, d);

    // Why: not readable from any API, so it is deployed unconditionally.
    push(
        &mut changes,
        ChangeKind::AlwaysApplied,
        "external_client_app.oauth.callback_url",
        "",
        &d.oauth.callback_url,
    );
    push(
        &mut changes,
        ChangeKind::AlwaysApplied,
        "external_client_app.oauth.pkce_required",
        "",
        &d.oauth.pkce_required.to_string(),
    );
    push(
        &mut changes,
        ChangeKind::AlwaysApplied,
        "external_client_app.oauth.consumer_secret_optional",
        "",
        &d.oauth.consumer_secret_optional.to_string(),
    );

    diff_permission_sets(
        &mut changes,
        &actual.permission_sets,
        &desired.permission_sets,
    );

    ChangeSet { changes }
}

fn diff_scopes(
    changes: &mut Vec<Change>,
    actual: &super::spec::ExternalClientApp,
    desired: &super::spec::ExternalClientApp,
) {
    let mut have = actual.oauth.scopes.clone();
    let mut want = desired.oauth.scopes.clone();
    have.sort_unstable();
    want.sort_unstable();

    for scope in &want {
        if !have.contains(scope) {
            push(
                changes,
                ChangeKind::Add,
                "external_client_app.oauth.scopes",
                "",
                scope.metadata_token(),
            );
        }
    }
    for scope in &have {
        if !want.contains(scope) {
            push(
                changes,
                ChangeKind::Remove,
                "external_client_app.oauth.scopes",
                scope.metadata_token(),
                "",
            );
        }
    }
    if actual.oauth.first_party_app_enabled != desired.oauth.first_party_app_enabled {
        push(
            changes,
            ChangeKind::Update,
            "external_client_app.oauth.first_party_app_enabled",
            &actual.oauth.first_party_app_enabled.to_string(),
            &desired.oauth.first_party_app_enabled.to_string(),
        );
    }
}

fn diff_policies(
    changes: &mut Vec<Change>,
    actual: &super::spec::ExternalClientApp,
    desired: &super::spec::ExternalClientApp,
) {
    let (a, d) = (&actual.policies, &desired.policies);
    compare_str(
        changes,
        "external_client_app.policies.permitted_users",
        &a.permitted_users,
        &d.permitted_users,
    );
    compare_str(
        changes,
        "external_client_app.policies.ip_relaxation",
        a.ip_relaxation.metadata_token(),
        d.ip_relaxation.metadata_token(),
    );
    compare_str(
        changes,
        "external_client_app.policies.refresh_token_policy",
        &a.refresh_token_policy,
        &d.refresh_token_policy,
    );
    let fmt_validity = |v: Option<&super::spec::Validity>| {
        v.map_or_else(
            || "none".to_owned(),
            |v| format!("{} {}", v.period, v.unit.metadata_token()),
        )
    };
    compare_str(
        changes,
        "external_client_app.policies.refresh_token_validity",
        &fmt_validity(a.refresh_token_validity.as_ref()),
        &fmt_validity(d.refresh_token_validity.as_ref()),
    );
    compare_str(
        changes,
        "external_client_app.policies.required_session_level",
        a.required_session_level.as_deref().unwrap_or(""),
        d.required_session_level.as_deref().unwrap_or(""),
    );
}

fn diff_permission_sets(
    changes: &mut Vec<Change>,
    actual: &[PermissionSetSpec],
    desired: &[PermissionSetSpec],
) {
    for want in desired {
        match actual.iter().find(|a| a.name == want.name) {
            None => push(changes, ChangeKind::Add, "permission_sets", "", &want.name),
            Some(have) => {
                compare_str(
                    changes,
                    &format!("permission_sets.{}.label", want.name),
                    &have.label,
                    &want.label,
                );
                compare_str(
                    changes,
                    &format!("permission_sets.{}.grants_app", want.name),
                    have.grants_app.as_deref().unwrap_or(""),
                    want.grants_app.as_deref().unwrap_or(""),
                );
            },
        }
    }
    for have in actual {
        if !desired.iter().any(|d| d.name == have.name) {
            push(
                changes,
                ChangeKind::Remove,
                "permission_sets",
                &have.name,
                "",
            );
        }
    }
}
