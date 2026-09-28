//! This suite's throwaway database.
//!
//! The harness is `internal-test-common`: one implementation, one `CI` guard.
//! Databases and templates are named after the running test binary, so this
//! suite's never collide with another's.

pub(crate) use internal_test_common::TempDb;
