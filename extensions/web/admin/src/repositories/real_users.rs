//! The definition of "is a real user", shared by the admin read paths.
//!
//! Anonymous visitors are stored as ordinary `users` rows — core's
//! `create_anonymous` writes `roles = ARRAY['anonymous']` and an
//! `<fingerprint>@anonymous.local` email — so any query that presents users as
//! people has to exclude them explicitly, and any code that inspects a row has
//! to agree with those queries about what it is looking at.

/// The canonical SQL predicate selecting rows that represent real people.
///
/// `sqlx`'s compile-time macros take SQL as a bare string literal and reject
/// `concat!`, so this cannot be spliced into a query — it is the reference text
/// that the copies in the query modules must match, and what
/// `scripts/check-anonymous-exclusion.sh` compares them against.
pub const REAL_USER_SQL: &str =
    "NOT ('anonymous' = ANY(u.roles)) AND u.email NOT LIKE '%@anonymous.local'";

/// The role core stamps on a visitor that has not signed in.
pub const ANONYMOUS_ROLE: &str = "anonymous";

/// The email domain core mints for such a visitor.
pub const ANONYMOUS_EMAIL_SUFFIX: &str = "@anonymous.local";

// Why: the two halves are checked together because either can survive alone —
// a row whose roles were edited keeps the minted email, and a row created by an
// older core carries the role without one.
pub fn is_anonymous<S: AsRef<str>>(roles: &[S], email: Option<&str>) -> bool {
    roles.iter().any(|r| r.as_ref() == ANONYMOUS_ROLE)
        || email.is_some_and(|e| e.ends_with(ANONYMOUS_EMAIL_SUFFIX))
}

pub fn is_real_user<S: AsRef<str>>(roles: &[S], email: Option<&str>) -> bool {
    !is_anonymous(roles, email)
}
