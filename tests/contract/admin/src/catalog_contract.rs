//! The catalog pages and the access-control JSON API behind them.
//!
//! Catalog entities come from `services/` on disk while their access rules
//! come from the database, so both sides are driven. Each API endpoint is
//! driven with a valid call, a wrong-shaped body, and an unknown path.

use axum::http::StatusCode;

use crate::app::{ADMIN_API_PREFIX, App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

fn api(path: &str) -> String {
    format!("{ADMIN_API_PREFIX}{path}")
}

#[tokio::test(flavor = "multi_thread")]
async fn catalog_pages_render_entities_from_the_profile() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        eprintln!("no DATABASE_URL — skipping catalog suite");
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let mut failures = Vec::new();
    for (path, expected) in [
        ("/admin/catalog", "/admin/plugins"),
        ("/admin/catalog/marketplace", "/admin/marketplaces"),
    ] {
        let (status, target) = app.redirect_of(Call::get(path, Principal::Admin)).await;
        if status != StatusCode::PERMANENT_REDIRECT {
            failures.push(format!("  {path} -> {} (expected 308)", status.as_u16()));
        } else if target != expected {
            failures.push(format!("  {path} redirected to {target:?}, not {expected}"));
        }
    }

    let listings: [(&str, &str); 3] = [
        (
            "/admin/plugins",
            "A plugin bundles skills, MCP servers, agents and hooks",
        ),
        ("/admin/skills", "The instruction sets people invoke"),
        ("/admin/mcp", "what it is serving right now"),
    ];
    for (path, marker) in listings {
        let (status, body) = app.call(Call::get(path, Principal::Admin)).await;
        if status != StatusCode::OK {
            failures.push(format!(
                "  {path} -> {} (expected 200): {}",
                status.as_u16(),
                body.chars().take(200).collect::<String>()
            ));
        } else if !body.contains(marker) {
            failures.push(format!("  {path} rendered without {marker:?}"));
        }
    }

    // Ids are read from the listing so the case survives a changed profile.
    for (listing, prefix) in [
        ("/admin/skills", "/admin/skills/"),
        ("/admin/mcp", "/admin/mcp/"),
        ("/admin/plugins", "/admin/plugins/"),
    ] {
        let (_, body) = app.call(Call::get(listing, Principal::Admin)).await;
        let Some(id) = first_detail_id(&body, prefix) else {
            continue;
        };
        let path = format!("{prefix}{id}");
        let (status, detail) = app.call(Call::get(&path, Principal::Admin)).await;
        if status != StatusCode::OK {
            failures.push(format!(
                "  {path} (an id the listing itself linked to) -> {} : {}",
                status.as_u16(),
                detail.chars().take(200).collect::<String>()
            ));
        } else if !detail.contains(&id) {
            failures.push(format!("  {path} rendered without naming {id:?}"));
        }
    }

    for path in [
        "/admin/skills/no-such-skill",
        "/admin/mcp/no-such-server",
        "/admin/plugins/no-such-plugin",
    ] {
        let (status, body) = app.call(Call::get(path, Principal::Admin)).await;
        if status.is_server_error() {
            failures.push(format!(
                "  {path} faulted: {}",
                body.chars().take(200).collect::<String>()
            ));
        }
    }

    db.cleanup().await;
    assert!(
        failures.is_empty(),
        "{} catalog page case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn first_detail_id(body: &str, prefix: &str) -> Option<String> {
    let needle = format!("href=\"{prefix}");
    let start = body.find(&needle)? + needle.len();
    let rest = &body[start..];
    let end = rest.find('"')?;
    let id = &rest[..end];
    (!id.is_empty() && !id.contains('/')).then(|| id.to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn entity_access_api_round_trips_a_grant() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let user_id = seed::unique("access-user");
    seed::insert_user(&db.pool, &user_id, &format!("{user_id}@contract.test")).await;
    let entity = seed::unique("skill-entity");

    let mut failures = Vec::new();

    let read = api(&format!("/access-control/entity/skill/{entity}/access"));
    let (status, body) = app.call(Call::get(&read, Principal::Admin)).await;
    if status != StatusCode::OK || !body.contains(r#""rules":[]"#) {
        failures.push(format!(
            "  reading an entity with no rules -> {} {}",
            status.as_u16(),
            body.chars().take(200).collect::<String>()
        ));
    }

    // A rule has a foreign key to `access_control_entities`; the default
    // endpoint creates that row, so it must run before the grant.
    let default_path = api(&format!("/access-control/entity/skill/{entity}/default"));
    let (status, _) = app
        .call(Call::json(
            "patch",
            &default_path,
            Principal::Admin,
            r#"{"default_included":false}"#,
        ))
        .await;
    if status != StatusCode::OK {
        failures.push(format!(
            "  registering the entity -> {} (expected 200)",
            status.as_u16()
        ));
    }

    let rules_path = api(&format!("/access-control/entity/skill/{entity}/rules"));
    let grant = format!(
        r#"{{"rule_type":"user","rule_value":"{user_id}","access":"allow","justification":"contract fixture"}}"#
    );
    let (status, body) = app
        .call(Call::json("post", &rules_path, Principal::Admin, &grant))
        .await;
    let rule_id = if status == StatusCode::OK {
        extract_json_string(&body, "\"id\":\"")
    } else {
        failures.push(format!(
            "  granting a user rule -> {} {}",
            status.as_u16(),
            body.chars().take(200).collect::<String>()
        ));
        None
    };

    let (_, body) = app.call(Call::get(&read, Principal::Admin)).await;
    if !body.contains(&user_id) {
        failures.push("  a granted rule did not read back on the entity".to_owned());
    }

    let (status, body) = app
        .call(Call::json(
            "patch",
            &default_path,
            Principal::Admin,
            r#"{"default_included":true}"#,
        ))
        .await;
    if status != StatusCode::OK || !body.contains(r#""default_included":true"#) {
        failures.push(format!(
            "  setting default_included -> {} {}",
            status.as_u16(),
            body.chars().take(200).collect::<String>()
        ));
    }

    if let Some(id) = rule_id {
        let delete_path = api(&format!("/access-control/entity/skill/{entity}/rules/{id}"));
        let (status, _) = app
            .call(Call::json("delete", &delete_path, Principal::Admin, "{}"))
            .await;
        if status != StatusCode::NO_CONTENT {
            failures.push(format!(
                "  deleting a rule -> {} (expected 204)",
                status.as_u16()
            ));
        }
        let (status, _) = app
            .call(Call::json("delete", &delete_path, Principal::Admin, "{}"))
            .await;
        if status != StatusCode::NOT_FOUND {
            failures.push(format!(
                "  deleting the same rule twice -> {} (expected 404)",
                status.as_u16()
            ));
        }
    }

    let rejected: [(&str, String, &str); 5] = [
        (
            "an unrecognised entity type",
            api("/access-control/entity/not-a-kind/x/rules"),
            r#"{"rule_type":"user","rule_value":"u","access":"allow"}"#,
        ),
        (
            "a rule type this form does not own",
            rules_path.clone(),
            r#"{"rule_type":"organization","rule_value":"acme","access":"allow"}"#,
        ),
        (
            "an access decision that is neither allow nor deny",
            rules_path.clone(),
            r#"{"rule_type":"user","rule_value":"u","access":"maybe"}"#,
        ),
        (
            "an empty rule value",
            rules_path.clone(),
            r#"{"rule_type":"user","rule_value":"   ","access":"allow"}"#,
        ),
        (
            "a body missing the access field entirely",
            rules_path.clone(),
            r#"{"rule_type":"user","rule_value":"u"}"#,
        ),
    ];
    for (label, path, body) in rejected {
        let (status, _) = app
            .call(Call::json("post", &path, Principal::Admin, body))
            .await;
        if !status.is_client_error() {
            failures.push(format!("  {label} -> {} (expected a 4xx)", status.as_u16()));
        }
    }

    for (label, path, want_ok) in [
        (
            "listing every gateway route's access",
            api("/access-control/entity-access/all?entity_type=gateway_route"),
            true,
        ),
        (
            "listing every MCP server's access",
            api("/access-control/entity-access/all?entity_type=mcp_server"),
            true,
        ),
        (
            "listing an entity type that is not a kind",
            api("/access-control/entity-access/all?entity_type=nonsense"),
            false,
        ),
        (
            "listing with no entity_type at all",
            api("/access-control/entity-access/all"),
            false,
        ),
    ] {
        let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
        if want_ok && status != StatusCode::OK {
            failures.push(format!(
                "  {label} -> {} : {}",
                status.as_u16(),
                body.chars().take(200).collect::<String>()
            ));
        }
        if !want_ok && !status.is_client_error() {
            failures.push(format!("  {label} -> {} (expected a 4xx)", status.as_u16()));
        }
    }

    db.cleanup().await;
    assert!(
        failures.is_empty(),
        "{} entity-access API case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn extract_json_string(body: &str, key: &str) -> Option<String> {
    let start = body.find(key)? + key.len();
    let rest = &body[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}
