#!/usr/bin/env python3
"""Generate the Cowork setup skills' bundled artifacts from services/artifacts/.

services/artifacts/<id>/{config.yaml,view.html} is the single source of truth
for every dashboard. Each setup skill ships a copy under assets/artifacts/
(<id>.html per dashboard plus manifest.json) because the Cowork VM can only
bash-copy files from the mounted skill directory. Hand-maintaining those
copies is how the copies diverged and how knowledge-feed went missing from
the bundle, so they are generated: the set of ids comes from the owning
plugin's artifacts.include list, the metadata from each artifact's
config.yaml, and the HTML is copied verbatim.

Usage:
  scripts/sync-cowork-artifacts.py            # regenerate the skill assets
  scripts/sync-cowork-artifacts.py --check    # exit 1 if regeneration would change anything
"""

import json
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent

# plugin whose artifacts.include defines the set -> skill that bundles it
PAIRS = {
    "systemprompt-crm": "systemprompt_setup_cowork",
    "systemprompt-admin": "admin_workspace_setup_cowork",
}


def plugin_artifact_ids(plugin_id):
    config = yaml.safe_load(
        (ROOT / "services/plugins" / plugin_id / "config.yaml").read_text()
    )
    selection = config["plugin"].get("artifacts") or {}
    if selection.get("source") != "explicit":
        raise SystemExit(
            f"{plugin_id}: artifacts selection must be explicit to be bundled"
        )
    return selection.get("include") or []


def manifest_record(artifact_id):
    src = ROOT / "services/artifacts" / artifact_id
    config = yaml.safe_load((src / "config.yaml").read_text())
    if config["id"] != artifact_id:
        raise SystemExit(
            f"{src}/config.yaml id {config['id']!r} != directory name {artifact_id!r}"
        )
    tools = config.get("mcp_tools") or []
    if not tools:
        raise SystemExit(f"{artifact_id}: empty mcp_tools would ship a dead dashboard")
    return {
        "id": config["id"],
        "name": config["name"],
        "description": config["description"],
        "version": config["version"],
        "isStarred": bool(config.get("starred", False)),
        "mcpTools": tools,
    }


def desired_state(plugin_id):
    files = {}
    records = []
    for artifact_id in plugin_artifact_ids(plugin_id):
        records.append(manifest_record(artifact_id))
        html = (ROOT / "services/artifacts" / artifact_id / "view.html").read_text()
        files[f"{artifact_id}.html"] = html
    files["manifest.json"] = (
        json.dumps({"artifacts": records}, indent=2, ensure_ascii=False) + "\n"
    )
    return files


def main():
    check = "--check" in sys.argv[1:]
    drift = []
    for plugin_id, skill_id in PAIRS.items():
        out_dir = ROOT / "services/skills" / skill_id / "assets/artifacts"
        desired = desired_state(plugin_id)
        existing = {p.name for p in out_dir.glob("*") if p.is_file()}
        for name in sorted(existing - set(desired)):
            drift.append(f"{skill_id}: stray {name}")
            if not check:
                (out_dir / name).unlink()
        for name, content in desired.items():
            path = out_dir / name
            if not path.exists() or path.read_text() != content:
                drift.append(f"{skill_id}: {name} out of date")
                if not check:
                    path.write_text(content)
    if drift:
        for line in drift:
            print(line)
        if check:
            print(
                "skill artifact assets have drifted from services/artifacts/ — "
                "run scripts/sync-cowork-artifacts.py"
            )
            return 1
        print(f"regenerated {len(drift)} file(s)")
    else:
        print("skill artifact assets are in sync")
    return 0


if __name__ == "__main__":
    sys.exit(main())
