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
    plugins.update((doc.get("plugins") or {}).keys())

known = {
    "skill": skills,
    "agent": agents,
    "mcp_server": mcp_servers,
    "marketplace": marketplaces,
    "plugin": plugins,
}
# gateway_route and hook entities are registered from the runtime profile at
# startup and cannot be resolved statically.
unresolvable = {"gateway_route", "hook"}

roles = load(root / "services/access-control/roles.yaml")
for rule in roles.get("rules") or []:
    etype = rule.get("entity_type")
    eid = rule.get("entity_id")
    if eid is None or etype in unresolvable:
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

if errors:
    print("services validation FAILED:")
    for e in errors:
        print(f"  {e}")
    sys.exit(1)
print("services validation OK")
EOF
