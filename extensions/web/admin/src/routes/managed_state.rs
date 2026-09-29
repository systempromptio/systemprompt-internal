//! Managed-resource services constructed once at the admin composition root.

use systemprompt::database::DbPool;
use systemprompt::marketplace::managed::ManagedRepository;

/// Why a state can fail to build: every core repository opens its own
/// handles from the shared [`DbPool`], and each constructor reports that.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error(transparent)]
    Managed(#[from] systemprompt::marketplace::managed::ManagedError),
}

#[derive(Debug, Clone)]
pub(crate) struct ManagedState {
    pub(crate) owner: systemprompt::identifiers::UserId,
    pub(crate) repository: ManagedRepository,
}

impl ManagedState {
    pub(crate) fn new(
        db: &DbPool,
        owner: systemprompt::identifiers::UserId,
    ) -> Result<Self, StateError> {
        Ok(Self {
            owner,
            repository: ManagedRepository::new(db)?,
        })
    }
}
