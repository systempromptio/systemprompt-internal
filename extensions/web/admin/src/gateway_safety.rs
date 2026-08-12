//! Gateway [`SafetyScanner`] implementation for the systemprompt template.
//!
//! [`SecretsScanner`] flags plaintext credentials (GitHub / Anthropic / AWS /
//! Stripe / … tokens, private keys, DB URLs with passwords) leaving the
//! gateway in a model reply, reusing the same `SECRET_PATTERNS` that the
//! governance chain applies on the way in. It registers through
//! `register_safety_scanner!` under the name `secrets`; the gateway runs it
//! for any policy whose `safety.scanners` lists it and blocks the reply when
//! `safety.block_response_categories` includes `secret`.
//!
//! **Egress only.** The `secret_scan` governance policy already scans request
//! content, runs first (`enforce_governance` precedes
//! `enforce_request_safety`), and is first-deny-wins, so scanning the request
//! here too produced a second verdict on identical bytes under different
//! config — two owners for one question, and a request that could be denied by
//! whichever plane the operator had not thought to configure. Responses have
//! no such overlap: the governance chain is request-only, so this is the sole
//! thing standing between a model that echoes a credential and the client.

use systemprompt::ai::{Finding, SafetyScanner, Severity, register_safety_scanner};
use systemprompt::models::wire::canonical::{CanonicalRequest, CanonicalResponse};

use systemprompt_security::policy::secrets::scan_str_for_secret;

#[derive(Debug, Clone, Copy, Default)]
pub struct SecretsScanner;

impl SecretsScanner {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SafetyScanner for SecretsScanner {
    fn name(&self) -> &'static str {
        "secrets"
    }

    async fn scan_request(&self, _req: &CanonicalRequest) -> Vec<Finding> {
        Vec::new()
    }

    async fn scan_response_final(&self, response: &CanonicalResponse) -> Vec<Finding> {
        let mut findings = Vec::new();
        for unit in response.content_units() {
            findings.extend(scan(&unit));
        }
        findings
    }
}

fn scan(text: &str) -> Vec<Finding> {
    scan_str_for_secret(text).map_or_else(Vec::new, |excerpt| {
        vec![Finding {
            phase: "response",
            severity: Severity::High,
            category: "secret".to_owned(),
            excerpt: Some(excerpt),
            scanner: "secrets",
        }]
    })
}

register_safety_scanner!(SecretsScanner::new, name = "secrets");


/// Blocks configurable banned words/phrases in request content. Registered as
/// an extension scanner rather than hardcoded in core, per the intended
/// architecture: core stays generic, client-specific word lists live here.
const BANNED_WORDS: &[&str] = &["duck"];

#[derive(Debug, Clone, Copy, Default)]
pub struct WordBlocklistScanner;

impl WordBlocklistScanner {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SafetyScanner for WordBlocklistScanner {
    fn name(&self) -> &'static str {
        "word_blocklist"
    }
    async fn scan_request(&self, req: &CanonicalRequest) -> Vec<Finding> {
        let mut findings = Vec::new();
        if let Some(text) = req.latest_message_text(systemprompt::models::wire::canonical::Role::User) {
            let lower = text.to_ascii_lowercase();
            for word in BANNED_WORDS {
                if lower.contains(word) {
                    findings.push(Finding {
                        phase: "request",
                        severity: Severity::Medium,
                        category: "word_blocklist".to_owned(),
                        excerpt: Some(word.to_string()),
                        scanner: "word_blocklist",
                    });
                }
            }
        }
        findings
    }
    async fn scan_response_final(&self, _response: &CanonicalResponse) -> Vec<Finding> {
        Vec::new()
    }
}

register_safety_scanner!(WordBlocklistScanner::new, name = "word_blocklist");
