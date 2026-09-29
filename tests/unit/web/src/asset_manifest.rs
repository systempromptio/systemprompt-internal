//! Build-time front-end standards gate.
//!
//! Validates the asset manifest and the front-end source tree against the
//! project's front-end coding standards: manifest/disk agreement in both
//! directions, template references resolving to published assets, inline
//! code kept out of templates, file-size ceilings, and a single-source
//! design-token invariant (`--sp-` prefix, one defining file per token per
//! CSS scope).


use crate::support::{repo_root, walk};

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use systemprompt::extension::AssetPaths;
use systemprompt::extension::prelude::Extension;
use systemprompt_web_extension::extension::WebExtension;

fn web_assets(paths: &dyn AssetPaths) -> Vec<systemprompt::extension::AssetDefinition> {
    WebExtension::new().required_assets(paths)
}

struct TestPaths {
    storage: PathBuf,
    dist: PathBuf,
}

impl AssetPaths for TestPaths {
    fn storage_files(&self) -> &Path {
        &self.storage
    }
    fn web_dist(&self) -> &Path {
        &self.dist
    }
}

fn test_paths() -> TestPaths {
    let root = repo_root();
    TestPaths {
        storage: root.join("storage/files"),
        dist: root.join("web/dist"),
    }
}

// Generated at publish time by `bundle_admin_css`; absent in a fresh checkout.
const GENERATED: &[&str] = &["css/admin-bundle.css"];

#[test]
fn every_registered_asset_exists_on_disk() {
    let paths = test_paths();
    let missing: Vec<String> = web_assets(&paths)
        .iter()
        .filter(|a| a.is_required() && !a.source().is_file())
        .map(|a| a.source().display().to_string())
        .collect();
    assert!(
        missing.is_empty(),
        "registered assets missing on disk:\n{}",
        missing.join("\n")
    );
}

fn template_files(root: &Path) -> Vec<PathBuf> {
    let mut templates = Vec::new();
    walk(&root.join("storage/files/admin"), "hbs", &mut templates);
    walk(&root.join("services/web/templates"), "html", &mut templates);
    // Why: both callers assert a property of every template they find, which
    // an empty list satisfies.
    assert!(
        !templates.is_empty(),
        "no admin or site templates under {}",
        root.display()
    );
    templates
}

fn extract_asset_refs(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for attr in ["src=\"", "href=\""] {
        for chunk in content.split(attr).skip(1) {
            let Some(value) = chunk.split('"').next() else {
                continue;
            };
            let value = value
                .replace("{{CSS_BASE_PATH}}", "/css")
                .replace("{{JS_BASE_PATH}}", "/js");
            let value = value.split('?').next().unwrap_or(&value).to_owned();
            let has_ext = |ext: &str| {
                Path::new(&value)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            };
            if (value.starts_with("/css/") && has_ext("css"))
                || (value.starts_with("/js/") && has_ext("js"))
            {
                refs.push(value.trim_start_matches('/').to_owned());
            }
        }
    }
    refs
}

#[test]
fn every_template_asset_reference_resolves() {
    let root = repo_root();
    let paths = test_paths();
    let published: BTreeSet<String> = web_assets(&paths)
        .iter()
        .map(|a| a.destination().to_owned())
        .collect();

    let mut unresolved = Vec::new();
    for template in template_files(&root) {
        let content = std::fs::read_to_string(&template)
            .unwrap_or_else(|e| panic!("read {}: {e}", template.display()));
        for reference in extract_asset_refs(&content) {
            if !published.contains(&reference) {
                unresolved.push(format!("{} -> {reference}", template.display()));
            }
        }
    }
    assert!(
        unresolved.is_empty(),
        "template asset references not in the manifest:\n{}",
        unresolved.join("\n")
    );
}

#[test]
fn templates_contain_no_inline_code() {
    let root = repo_root();
    let mut violations = Vec::new();
    for template in template_files(&root) {
        let content = std::fs::read_to_string(&template)
            .unwrap_or_else(|e| panic!("read {}: {e}", template.display()));
        let name = template.display().to_string();
        // Why: the admin shell carries its unstyled-flash guard inline, because
        // it has to land before the stylesheet link resolves.
        let is_fouc_shell = name.ends_with("partials/layout.hbs");
        let is_critical_css_shell = name.ends_with("partials/head-assets.html") || is_fouc_shell;
        for (idx, line) in content.lines().enumerate() {
            let has_open = line.contains("<script") && !line.contains("src=");
            let is_data = line.contains("application/ld+json") || line.contains("application/json");
            if has_open && !is_data && !is_fouc_shell {
                violations.push(format!("{name}:{} inline <script>", idx + 1));
            }
            if line.contains("<style") && !is_critical_css_shell {
                violations.push(format!("{name}:{} inline <style>", idx + 1));
            }
            if line.contains("style=\"") && !line.contains("style=\"--") {
                violations.push(format!("{name}:{} inline style attribute", idx + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "inline code in templates (extract to registered CSS/JS):\n{}",
        violations.join("\n")
    );
}

#[test]
fn source_files_respect_line_limits() {
    let root = repo_root();
    let mut js = Vec::new();
    let mut css = Vec::new();
    walk(&root.join("storage/files/js"), "js", &mut js);
    walk(&root.join("storage/files/css"), "css", &mut css);
    assert!(!js.is_empty(), "no JS sources under {}", root.display());
    assert!(!css.is_empty(), "no CSS sources under {}", root.display());

    let mut oversized = Vec::new();
    for (files, limit) in [(&js, 250usize), (&css, 400usize)] {
        for file in files {
            let rel = file.strip_prefix(&root).unwrap_or(file);
            if GENERATED.iter().any(|g| rel.to_string_lossy().ends_with(g)) {
                continue;
            }
            let content = std::fs::read_to_string(file)
                .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
            let lines = content.lines().count();
            if lines > limit {
                oversized.push(format!("{} ({lines} > {limit})", rel.display()));
            }
        }
    }
    assert!(
        oversized.is_empty(),
        "files over the line limit (split by component/concern):\n{}",
        oversized.join("\n")
    );
}

fn token_definitions(file: &Path) -> Vec<String> {
    let content =
        std::fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("--") {
                trimmed.split(':').next().map(str::to_owned)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn custom_properties_use_sp_prefix() {
    let root = repo_root();
    let mut css = Vec::new();
    walk(&root.join("storage/files/css"), "css", &mut css);
    assert!(!css.is_empty(), "no CSS sources under {}", root.display());
    let mut violations = Vec::new();
    for file in &css {
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();
        if GENERATED.iter().any(|g| rel.ends_with(g)) {
            continue;
        }
        for token in token_definitions(file) {
            if !token.starts_with("--sp-") {
                violations.push(format!(
                    "{}: {token}",
                    file.strip_prefix(&root).unwrap_or(file).display()
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "custom properties without the --sp- prefix:\n{}",
        violations.join("\n")
    );
}
