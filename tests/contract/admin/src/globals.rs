//! Process-wide globals the admin handlers read, installed once per test
//! binary.
//!
//! Core's config, profile, secrets and signing authority are `OnceLock`s, so
//! they are installed behind a `Once` and every token minted in
//! [`crate::principal`] validates against the same issuer. The profile is a
//! checked-in fixture: the developer's `.systemprompt` profile is git-ignored,
//! so relying on it would pass locally and silently skip in CI.

use std::path::PathBuf;
use std::sync::{Once, OnceLock};

static INIT: Once = Once::new();
static READY: OnceLock<bool> = OnceLock::new();

const ISSUER: &str = "http://localhost:8099";

pub(crate) use internal_test_common::repo_root;

pub(crate) fn init() -> bool {
    INIT.call_once(|| {
        let ready = try_init();
        READY
            .set(ready)
            .expect("call_once runs this exactly once, so READY is unset");
    });
    *READY.get().unwrap_or(&false)
}

fn try_init() -> bool {
    // Why: a 500 renders a body with no cause; the handler logs the cause at
    // ERROR, and the test writer prints it with the failing test's output.
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::ERROR)
        .with_test_writer()
        .try_init();
    let profile = write_fixture_profile();

    systemprompt::config::ProfileBootstrap::init_from_path(&profile)
        .expect("initialise the contract fixture profile");
    // Why: tokio panics on `block_on` inside a `#[tokio::test]` runtime, so the
    // async secrets bootstrap runs on a throwaway runtime on its own thread.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("runtime for the secrets bootstrap")
                    .block_on(systemprompt::config::SecretsBootstrap::try_init())
                    .expect("load the fixture profile's secrets");
            })
            .join()
            .expect("the secrets bootstrap thread did not panic");
    });
    systemprompt::config::try_init_config(None).expect("build config from the fixture profile");
    systemprompt::loader::ServicesBootstrap::try_init()
        .expect("load the services tree the fixture profile points at");

    let key = systemprompt_security::keys::RsaSigningKey::generate()
        .expect("generate an ephemeral RSA signing key");
    systemprompt_security::keys::authority::install_for_test(key);
    true
}

const FIXTURE_PROFILE: &str = include_str!("../fixtures/profile.yaml");

// `anthropic` must exist: core's services loader demotes a provider with no
// credential to `surface: backend`, which hides both fixture routes from the
// per-user catalog before any ACL rule is consulted.
fn fixture_secrets() -> String {
    let database_url = std::env::var("SYSTEMPROMPT_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://unused:unused@localhost:5432/postgres".to_owned());
    format!(
        r#"{{
  "database_url": "{database_url}",
  "oauth_at_rest_pepper": "contract-suite-pepper-not-a-real-secret",
  "mcp_credential_broker_secret": "contract-suite-broker-not-a-real-secret",
  "manifest_signing_secret_seed": "Y29udHJhY3Qtc3VpdGUtc2VlZC1ub3QtcmVhbC0wMDA=",
  "encryption_master_key": "{MASTER_KEY_HEX}",
  "anthropic": "contract-suite-anthropic-key-not-a-real-secret"
}}"#
    )
}

// `secret_crypto::load_master_key` refuses anything but exactly 32 bytes.
const MASTER_KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

// Profile path validation requires `paths.bin` to exist; a runner that built
// only this workspace has no `target/debug`, so an empty `bin/` stands in.
fn write_fixture_profile() -> PathBuf {
    let root = repo_root();
    // Why: the tree is rewritten on every init; two concurrent runs sharing a
    // path race into "No such file or directory" mid-copy.
    let dir = root.join(format!(
        "tests/target/contract-profile-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("bin")).expect("create fixture profile and bin directories");

    let yaml = FIXTURE_PROFILE
        .replace("__PROFILE_DIR__", &dir.to_string_lossy())
        .replace("__REPO__", &root.to_string_lossy())
        .replace(
            "jwt_issuer: http://localhost:8099",
            &format!("jwt_issuer: {ISSUER}"),
        );
    std::fs::write(dir.join("profile.yaml"), yaml).expect("write fixture profile");
    std::fs::write(dir.join("secrets.json"), fixture_secrets()).expect("write fixture secrets");
    write_fixture_services(&root, &dir.join("services"));

    dir.join("profile.yaml")
}

// Two routes with fixed ids and no default provider, so an unrouted model
// exists for the ACL detector cases; the rest of `services/` is copied as-is.
const FIXTURE_PROVIDERS: &str = include_str!("../fixtures/providers.yaml");
const FIXTURE_GATEWAY: &str = include_str!("../fixtures/gateway.yaml");

fn write_fixture_services(root: &std::path::Path, dest: &std::path::Path) {
    if dest.exists() {
        std::fs::remove_dir_all(dest).expect("clear the previous fixture services tree");
    }
    copy_tree(&root.join("services"), dest);
    std::fs::write(dest.join("ai/providers.yaml"), FIXTURE_PROVIDERS)
        .expect("write fixture providers.yaml");
    std::fs::write(dest.join("ai/gateway.yaml"), FIXTURE_GATEWAY)
        .expect("write fixture gateway.yaml");
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("create fixture services directory");
    for entry in std::fs::read_dir(from).expect("read services directory") {
        let entry = entry.expect("read services entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy services file");
        }
    }
}

pub(crate) fn jwt_issuer() -> String {
    systemprompt::models::Config::get()
        .expect("config installed by init()")
        .jwt_issuer
        .clone()
}
