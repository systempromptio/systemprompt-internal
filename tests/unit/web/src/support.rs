//! Helpers shared by the front-end standards gates.

use std::path::PathBuf;

// The repository root, found by climbing from this crate until the directory
// holding the front-end sources these gates read appears.
//
// Why the search rather than a fixed number of `pop()`s: this crate's depth
// below the root is not a fact the gates should depend on. A hard-coded depth
// that goes stale resolves to a directory that simply has no templates or
// assets in it, and every gate then walks an empty tree and passes — the one
// failure mode a gate must never have.
pub(crate) fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("storage/files").is_dir() && dir.join("scripts").is_dir() {
            return dir;
        }
        assert!(
            dir.pop(),
            "repository root not found above CARGO_MANIFEST_DIR"
        );
    }
}
