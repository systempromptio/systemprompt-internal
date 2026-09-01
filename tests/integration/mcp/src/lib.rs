//! Integration coverage for `systemprompt-mcp-shared`'s audit writers against a
//! live Postgres, focused on the fail-closed rejection path
//! (`record_mcp_access_rejected`): a rejection audit row is written only when a
//! reserved anonymous principal exists to attribute it to, and is silently
//! dropped (no row, no error) otherwise — plus the approval rendezvous and the
//! brain@ proposal state machine that rides on it.
//!
//! Every test runs against its OWN throwaway database created on the live
//! server named by `DATABASE_URL`, so the shared application tables are never
//! read, written, or truncated. The database is dropped on completion.

#[cfg(test)]
mod approval_gate;
#[cfg(test)]
mod common;
#[cfg(test)]
mod knowledge_proposal_state;
#[cfg(test)]
mod record_access;
#[cfg(test)]
mod rejection_fail_closed;
