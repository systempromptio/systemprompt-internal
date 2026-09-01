#!/usr/bin/env bash
# Cross-file referential integrity for the services/ YAML tree.
#
# Catches at commit time what otherwise only fails (or silently stops
# matching) at boot: access-control rules pointing at ids that no resource
# defines, and MCP port declarations drifting between services/mcp/ and the
# extension manifest.
set -uo pipefail
cd "$(dirname "$0")/.."

python3 - <<'EOF'
import pathlib
import sys

import yaml

root = pathlib.Path(".")
errors = []


def load(path):
    try:
        return yaml.safe_load(path.read_text()) or {}
    except yaml.YAMLError as e:
        errors.append(f"{path}: unparseable YAML: {e}")
        return {}


skills = {
    load(p).get("id")
    for p in root.glob("services/skills/*/config.yaml")
}
agents = set()
for p in root.glob("services/agents/*.yaml"):
    agents.update((load(p).get("agents") or {}).keys())
mcp_servers = set()
for p in root.glob("services/mcp/*.yaml"):
    mcp_servers.update((load(p).get("mcp_servers") or {}).keys())
marketplaces = {
    (load(p).get("marketplace") or {}).get("id")
    for p in root.glob("services/marketplaces/*/config.yaml")
}
plugins = set()
for p in root.glob("services/plugins/*/config.yaml"):
    doc = load(p)
    # Two shapes are in use: a `plugins:` map keyed by id, and a single
    # `plugin:` block that carries its own `id`. Both name the same thing.
    plugins.update((doc.get("plugins") or {}).keys())
    single = (doc.get("plugin") or {}).get("id")
    if single:
        plugins.add(single)

known = {
    "skill": skills,
    "agent": agents,
    "mcp_server": mcp_servers,
    "marketplace": marketplaces,
    "plugin": plugins,
}
# Nothing registers a `hook` entity: no loader or bootstrap writes that kind,
# so a literal hook id has no catalog to be checked against — and, like a
# gateway_route id, it would be minted rather than validated. A hook rule may
# use entity_match; a literal id is rejected below with the route ids.
minted_not_validated = {"gateway_route", "hook"}

roles = load(root / "services/access-control/roles.yaml")
for rule in roles.get("rules") or []:
    etype = rule.get("entity_type")
    eid = rule.get("entity_id")
    if eid is None:
        continue
    # A literal gateway_route id cannot be validated here (profiles are
    # gitignored, so CI has no route list) and cannot be correct either: route
    # ids are generated as synthesize_route_id(model_pattern, provider), so no
    # hand-written id matches a real route. Reject the practice rather than the
    # value — that needs no profile.
    if etype in minted_not_validated:
        errors.append(
            f"roles.yaml: {etype} rules must use entity_match, not a literal "
            f"entity_id ('{eid}') — no catalog registers a written-out {etype} id, "
            f"so it would be minted, not checked"
        )
        continue
    pool = known.get(etype)
    if pool is None:
        errors.append(f"roles.yaml: unknown entity_type '{etype}' on '{eid}'")
    elif eid not in pool:
        errors.append(
            f"roles.yaml: entity_id '{eid}' (type {etype}) matches no defined resource"
        )

for svc_path in root.glob("services/mcp/*.yaml"):
    for name, cfg in (load(svc_path).get("mcp_servers") or {}).items():
        manifest_path = root / "extensions/mcp" / name / "manifest.yaml"
        if not manifest_path.is_file():
            continue
        service_port = cfg.get("port")
        manifest_port = (load(manifest_path).get("extension") or {}).get("port")
        if manifest_port is not None and service_port != manifest_port:
            errors.append(
                f"{svc_path}: port {service_port} disagrees with "
                f"{manifest_path}: port {manifest_port}"
            )

# The checked-in marketplace JSON under storage/files/plugins/.claude-plugin/
# is generated from services config (core: plugins/generate/marketplace.rs).
# It went stale once (phantom per-plugin agents survived a config rewrite), so
# pin its plugin list and version to the marketplace config here.
import json

for mp_path in root.glob("services/marketplaces/*/config.yaml"):
    mp = (load(mp_path) or {}).get("marketplace") or {}
    mp_id = mp.get("id")
    json_path = (
        root / "storage/files/plugins/.claude-plugin" / f"marketplace-{mp_id}.json"
    )
    if not json_path.is_file():
        continue
    generated = json.loads(json_path.read_text())
    declared = list((mp.get("plugins") or {}).get("include") or [])
    emitted = [p.get("name") for p in generated.get("plugins") or []]
    if declared != emitted:
        errors.append(
            f"{json_path}: plugin list {emitted} is stale — marketplace config "
            f"declares {declared}; regenerate the marketplace JSON"
        )
    declared_version = mp.get("version")
    emitted_version = (generated.get("metadata") or {}).get("version")
    if declared_version != emitted_version:
        errors.append(
            f"{json_path}: version {emitted_version} is stale — marketplace "
            f"config declares {declared_version}"
        )

# ---------------------------------------------------------------------------
# Plugin scope invariants. A plugin is the role boundary: skills and artifacts
# inherit their plugin's access rule, the plugin inherits the marketplace's,
# and the nearest declared level decides. Everything below keeps that model
# true at commit time, so it cannot drift back into per-skill rules, mixed
# plugins, orphaned skills, or dashboards pointing at a server that is off.
# ---------------------------------------------------------------------------

def is_enabled(doc):
    return bool(doc.get("enabled", True))


plugin_docs = {}
for p in root.glob("services/plugins/*/config.yaml"):
    doc = load(p)
    for pid, body in ((doc.get("plugins") or {})).items():
        plugin_docs[pid] = (body or {}, p)
    single = doc.get("plugin") or {}
    if single.get("id"):
        plugin_docs[single["id"]] = (single, p)

skill_docs = {}
for p in root.glob("services/skills/*/config.yaml"):
    doc = load(p)
    if doc.get("id"):
        skill_docs[doc["id"]] = (doc, p)

artifact_docs = {}
for p in root.glob("services/artifacts/*/config.yaml"):
    doc = load(p)
    if doc.get("id"):
        artifact_docs[doc["id"]] = (doc, p)

mcp_docs = {}
for p in root.glob("services/mcp/*.yaml"):
    for name, cfg in (load(p).get("mcp_servers") or {}).items():
        mcp_docs[name] = (cfg or {}, p)

rules = roles.get("rules") or []
plugin_rules = {}
for rule in rules:
    if rule.get("entity_type") == "plugin" and rule.get("entity_id"):
        plugin_rules.setdefault(rule["entity_id"], []).append(rule)

admin_only_mcp = {
    r["entity_id"]
    for r in rules
    if r.get("entity_type") == "mcp_server"
    and r.get("entity_id")
    and r.get("access", "allow") == "allow"
    and set(r.get("roles") or []) == {"admin"}
}


def selection(body, key):
    sel = body.get(key) or {}
    if isinstance(sel, dict):
        return list(sel.get("include") or [])
    return []


# 1–2. Every plugin declares exactly one scope, and the sentinel matches it.
plugin_scope = {}
for pid, (body, path) in sorted(plugin_docs.items()):
    declared = plugin_rules.get(pid, [])
    allows = [r for r in declared if r.get("access", "allow") == "allow"]
    if len(allows) != 1:
        errors.append(
            f"{path}: plugin '{pid}' must declare exactly one entity_type: plugin allow "
            f"rule in roles.yaml (found {len(allows)}) — that rule is its role scope"
        )
        continue
    rule = allows[0]
    rr = set(rule.get("roles") or [])
    if rr == {"admin"}:
        scope = "admin"
    elif "user" in rr:
        scope = "user"
    else:
        errors.append(
            f"roles.yaml: plugin '{pid}' roles {sorted(rr)} name neither 'user' nor "
            f"exactly ['admin'] — scope must be user (shared by every role) or admin"
        )
        continue
    plugin_scope[pid] = scope
    want_default = scope == "user"
    if bool(rule.get("default_included", False)) != want_default:
        errors.append(
            f"roles.yaml: plugin '{pid}' is {scope}-scoped, so default_included must be "
            f"{str(want_default).lower()}"
        )

# 3. Every enabled plugin's members exist and are enabled; 7. admin servers stay
#    out of user plugins.
for pid, (body, path) in sorted(plugin_docs.items()):
    if not is_enabled(body):
        continue
    for sid in selection(body, "skills"):
        if sid not in skill_docs:
            errors.append(f"{path}: plugin '{pid}' includes unknown skill '{sid}'")
        elif not is_enabled(skill_docs[sid][0]):
            errors.append(f"{path}: plugin '{pid}' includes disabled skill '{sid}'")
    for aid in selection(body, "artifacts"):
        if aid not in artifact_docs:
            errors.append(f"{path}: plugin '{pid}' includes unknown artifact '{aid}'")
        elif not is_enabled(artifact_docs[aid][0]):
            errors.append(f"{path}: plugin '{pid}' includes disabled artifact '{aid}'")
    for mid in selection(body, "mcp_servers"):
        if mid not in mcp_docs:
            errors.append(f"{path}: plugin '{pid}' includes unknown mcp_server '{mid}'")
        elif not is_enabled(mcp_docs[mid][0]):
            errors.append(
                f"{path}: plugin '{pid}' is enabled but depends on disabled mcp_server "
                f"'{mid}' — enable the server or disable the plugin"
            )
        elif plugin_scope.get(pid) == "user" and mid in admin_only_mcp:
            errors.append(
                f"{path}: user-scoped plugin '{pid}' includes admin-only mcp_server "
                f"'{mid}' — its users could never call it"
            )

# 4. Orphans fail: every enabled skill and artifact is shipped by an enabled plugin.
shipped_skills = {}
shipped_artifacts = {}
for pid, (body, path) in plugin_docs.items():
    if not is_enabled(body):
        continue
    for sid in selection(body, "skills"):
        shipped_skills.setdefault(sid, set()).add(pid)
    for aid in selection(body, "artifacts"):
        shipped_artifacts.setdefault(aid, set()).add(pid)
for sid, (doc, path) in sorted(skill_docs.items()):
    if is_enabled(doc) and sid not in shipped_skills:
        errors.append(
            f"{path}: skill '{sid}' is enabled but no enabled plugin includes it — it "
            f"reaches no client; add it to a plugin or set enabled: false"
        )
for aid, (doc, path) in sorted(artifact_docs.items()):
    if is_enabled(doc) and aid not in shipped_artifacts:
        errors.append(
            f"{path}: artifact '{aid}' is enabled but no enabled plugin includes it — "
            f"it reaches no client; add it to a plugin or set enabled: false"
        )

# 5. Skills inherit their plugin: an allow-type skill rule is the drift this
#    model removes. A deny must target a shipped skill.
for rule in rules:
    if rule.get("entity_type") != "skill" or not rule.get("entity_id"):
        continue
    sid = rule["entity_id"]
    if rule.get("access", "allow") == "allow":
        errors.append(
            f"roles.yaml: skill '{sid}' carries an allow rule — skills inherit their "
            f"plugin's rule; move the grant to the plugin (or use access: deny to "
            f"exclude one skill)"
        )
    elif sid not in shipped_skills:
        errors.append(
            f"roles.yaml: skill deny on '{sid}' names a skill no enabled plugin ships"
        )

# 6. Exactly one enabled plugin owns the session-global governance hooks.
owners = [
    pid
    for pid, (body, _) in plugin_docs.items()
    if is_enabled(body) and bool((body.get("hooks") or {}).get("governance"))
]
if len(owners) != 1:
    errors.append(
        f"services/plugins: exactly one enabled plugin must set hooks.governance: true "
        f"(found {owners or 'none'})"
    )

# 8. Every artifact's tools name a server that exists and is enabled, and an
#    artifact is never split across scopes.
for aid, (doc, path) in sorted(artifact_docs.items()):
    if not is_enabled(doc):
        continue
    for tool in doc.get("mcp_tools") or []:
        parts = tool.split("__")
        server = parts[1] if tool.startswith("mcp__") and len(parts) >= 3 else None
        if not server:
            errors.append(f"{path}: mcp_tools entry '{tool}' is not mcp__<server>__<tool>")
        elif server not in mcp_docs:
            errors.append(f"{path}: mcp_tools entry '{tool}' names unknown mcp_server '{server}'")
        elif not is_enabled(mcp_docs[server][0]):
            errors.append(
                f"{path}: artifact '{aid}' depends on disabled mcp_server '{server}' — "
                f"enable the server or disable the artifact"
            )
    scopes = {plugin_scope.get(pid) for pid in shipped_artifacts.get(aid, set())}
    scopes.discard(None)
    if len(scopes) > 1:
        errors.append(
            f"{path}: artifact '{aid}' is shipped by plugins of different scopes "
            f"{sorted(scopes)} — pick one owner scope"
        )

# 9. The marketplace names every enabled plugin, and only real ones.
for mp_path in root.glob("services/marketplaces/*/config.yaml"):
    mp = (load(mp_path) or {}).get("marketplace") or {}
    if not is_enabled(mp):
        continue
    included = list((mp.get("plugins") or {}).get("include") or [])
    for pid in included:
        if pid not in plugin_docs:
            errors.append(f"{mp_path}: plugins.include names unknown plugin '{pid}'")
    for pid, (body, _) in sorted(plugin_docs.items()):
        if is_enabled(body) and included and pid not in included:
            errors.append(
                f"{mp_path}: enabled plugin '{pid}' is not in plugins.include — it ships "
                f"nowhere"
            )
    for mid in list((mp.get("mcp_servers") or {}).get("include") or []):
        if mid in mcp_docs and not is_enabled(mcp_docs[mid][0]):
            errors.append(
                f"{mp_path}: mcp_servers.include names disabled mcp_server '{mid}'"
            )

if errors:
    print("services validation FAILED:")
    for e in errors:
        print(f"  {e}")
    sys.exit(1)
print("services validation OK")
EOF
