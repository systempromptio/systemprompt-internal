#!/usr/bin/env bash
# Cross-file referential integrity for the services/ YAML tree.
#
# Catches at commit time what otherwise only fails (or silently stops
# matching) at boot: access-control rules pointing at ids that no resource
# defines, plugins and marketplaces including members no file declares (the
# composition rule ServicesConfig::validate() enforces — a dangling include
# stops the tree composing), and MCP port declarations drifting between
# services/mcp/ and the extension manifest.
#
# Entitlement is declared in services/access-control/rules.yaml (checked
# below), the one file the server seeds access control from.
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

# Cross-file includes. A plugin lists agents by id; a marketplace lists MCP
# servers by id. Composition refuses an id nothing declares. (Skills,
# artifacts and a plugin's MCP servers are checked with their enabled state in
# the plugin-scope invariants below.)
def includes(block):
    block = block or {}
    if not isinstance(block, dict) or block.get("source", "explicit") != "explicit":
        return []
    return block.get("include") or []


for p in root.glob("services/plugins/*/config.yaml"):
    doc = load(p)
    blocks = list((doc.get("plugins") or {}).values())
    if doc.get("plugin"):
        blocks.append(doc["plugin"])
    for block in blocks:
        pid = block.get("id", p.parent.name)
        for member in includes(block.get("agents")):
            if member not in agents:
                errors.append(
                    f"{p}: plugin '{pid}': agents.include references unknown agent '{member}'"
                )

for p in root.glob("services/marketplaces/*/config.yaml"):
    mp = load(p).get("marketplace") or {}
    if not mp.get("id"):
        errors.append(f"{p}: marketplace declares no id")
    for member in includes(mp.get("mcp_servers")):
        if member not in mcp_servers:
            errors.append(
                f"{p}: marketplace '{mp.get('id')}': mcp_servers.include references "
                f"unknown mcp_server '{member}'"
            )

known = {
    "skill": skills,
    "agent": agents,
    "mcp_server": mcp_servers,
    "marketplace": marketplaces,
    "plugin": plugins,
}
# services/access-control/rules.yaml is the ONE declarative source of
# entitlement. Every entity it names must exist, every group/project it names
# must be declared, every entity must say why, and no marketplace config may
# carry an `access:` block of its own — that second truth is exactly what this
# file replaced.
RULES = root / "services/access-control/rules.yaml"
BANDS = {"role", "group", "project", "connector"}
# Nothing registers a `hook` entity, gateway_route ids are generated and Slack
# channel ids are Slack's, so a literal id of any of them would be minted
# rather than validated. Only the glob is accepted for them, and only for them.
glob_only = {"gateway_route", "hook", "slack_channel"}

groups_doc = load(root / "services/web/config/groups.yaml")
group_ids = {g.get("id") for g in (groups_doc.get("groups") or [])} | {"unassigned"}
project_ids = {p.get("id") for p in (groups_doc.get("projects") or [])}
member_ids = {"group": group_ids, "project": project_ids}


def band_values(spec):
    if isinstance(spec, dict):
        return spec.get("values") or [], spec.get("why")
    return spec or [], None


rules_doc = load(RULES) if RULES.exists() else {}
if not RULES.exists():
    errors.append(f"{RULES}: missing — it is the one declarative source of entitlement")
declared_entities = {}
for decl in rules_doc.get("entities") or []:
    ref = str(decl.get("entity", ""))
    if "/" not in ref:
        errors.append(f"rules.yaml: entity '{ref}' must be written as <kind>/<id>")
        continue
    if ref in declared_entities:
        errors.append(f"rules.yaml: {ref}: declared twice")
    declared_entities[ref] = decl
    etype, eid = ref.split("/", 1)
    if not str(decl.get("why") or "").strip():
        errors.append(f"rules.yaml: {ref}: `why` is required")
    if decl.get("default", "closed") not in ("open", "closed"):
        errors.append(f"rules.yaml: {ref}: default must be open or closed")
    if etype in glob_only:
        if eid != "*":
            errors.append(
                f"rules.yaml: {ref}: {etype} ids are generated, never written — use {etype}/*"
            )
    elif "*" in eid:
        errors.append(f"rules.yaml: {ref}: only {sorted(glob_only)} take a glob")
    else:
        pool = known.get(etype)
        owner = decl.get("owner")
        # An entity a remote bundle owns (`owner: bundle:<name>`) may be absent
        # from this tree: composition forbids an id both local and bundled, so
        # the kit's marketplace is declared here and arrives with the bundle.
        if owner is not None:
            if not (isinstance(owner, str) and owner.startswith("bundle:") and owner[7:].strip()):
                errors.append(f"rules.yaml: {ref}: owner must be written as bundle:<name>")
            if etype not in ("marketplace", "plugin", "skill"):
                errors.append(f"rules.yaml: {ref}: only a marketplace, plugin or skill can name an owner")
            if pool is not None and eid in pool:
                errors.append(
                    f"rules.yaml: {ref}: is defined in this tree and names owner {owner} — "
                    f"composition refuses an id that is both local and bundled"
                )
        elif pool is None:
            errors.append(f"rules.yaml: {ref}: unknown entity kind '{etype}'")
        elif eid not in pool:
            errors.append(f"rules.yaml: {ref}: matches no defined resource")
    allow = decl.get("allow") or {}
    deny = decl.get("deny") or {}
    if not allow and not deny:
        errors.append(f"rules.yaml: {ref}: declares no allow and no deny")
    for verb, bands in (("allow", allow), ("deny", deny)):
        for band, spec in bands.items():
            if band not in BANDS:
                errors.append(f"rules.yaml: {ref}: unknown band '{band}' under {verb}")
                continue
            values, why = band_values(spec)
            if not values:
                errors.append(f"rules.yaml: {ref}: {verb}.{band} names no subjects")
            if isinstance(spec, dict) and not str(why or "").strip():
                errors.append(f"rules.yaml: {ref}: {verb}.{band} has a `why` key that is empty")
            for value in values:
                if band in member_ids and value not in member_ids[band]:
                    errors.append(
                        f"rules.yaml: {ref}: {band} '{value}' is not declared in "
                        f"services/web/config/groups.yaml"
                    )
    for band in set(allow) & set(deny):
        both = set(band_values(allow[band])[0]) & set(band_values(deny[band])[0])
        for value in sorted(both):
            errors.append(f"rules.yaml: {ref}: {band} '{value}' is both allowed and denied")

for p in root.glob("services/marketplaces/*/config.yaml"):
    if "access" in (load(p).get("marketplace") or {}):
        errors.append(
            f"{p}: marketplace configs carry no `access:` block — declare "
            f"marketplace/<id> in services/access-control/rules.yaml instead"
        )


def role_set(decl, verb):
    return set(band_values((decl.get(verb) or {}).get("role"))[0])


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

plugin_decls = {
    ref.split("/", 1)[1]: decl
    for ref, decl in declared_entities.items()
    if ref.startswith("plugin/")
}

admin_only_mcp = {
    ref.split("/", 1)[1]
    for ref, decl in declared_entities.items()
    if ref.startswith("mcp_server/") and role_set(decl, "allow") == {"admin"}
}


def selection(body, key):
    sel = body.get(key) or {}
    if isinstance(sel, dict):
        return list(sel.get("include") or [])
    return []


# 1–2. Every plugin declares exactly one scope, and its default matches it.
plugin_scope = {}
for pid, (body, path) in sorted(plugin_docs.items()):
    decl = plugin_decls.get(pid)
    if decl is None:
        errors.append(
            f"{path}: plugin '{pid}' must be declared as plugin/{pid} in rules.yaml "
            f"with a role allow — that entity is its role scope"
        )
        continue
    rr = role_set(decl, "allow")
    if rr == {"admin"}:
        scope = "admin"
    elif "user" in rr:
        scope = "user"
    else:
        errors.append(
            f"rules.yaml: plugin/{pid} allow.role {sorted(rr)} names neither 'user' nor "
            f"exactly ['admin'] — scope must be user (shared by every role) or admin"
        )
        continue
    plugin_scope[pid] = scope
    want_default = "open" if scope == "user" else "closed"
    if decl.get("default", "closed") != want_default:
        errors.append(
            f"rules.yaml: plugin/{pid} is {scope}-scoped, so default must be {want_default}"
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

# 5. Skills inherit their plugin: an allow on a skill entity is the drift this
#    model removes. A deny must target a shipped skill.
for ref, decl in declared_entities.items():
    if not ref.startswith("skill/") or decl.get("owner"):
        continue
    sid = ref.split("/", 1)[1]
    if decl.get("allow"):
        errors.append(
            f"rules.yaml: {ref} carries an allow — skills inherit their plugin's rule; "
            f"move the grant to the plugin (or use deny to exclude one skill)"
        )
    elif sid not in shipped_skills:
        errors.append(f"rules.yaml: {ref}: deny names a skill no enabled plugin ships")

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
