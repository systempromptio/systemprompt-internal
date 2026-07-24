#!/usr/bin/env python3
"""Stage each setup skill's dashboards into that skill's assets directory.

The Cowork plugin bundler ships a skill's aux subdirs (assets/, references/,
...) verbatim, and Cowork mounts them read-only inside the session VM. Copying
each dashboard's HTML here lets a setup skill install artifacts with a
byte-exact `cp` instead of re-typing ~100KB of HTML through the model.

Which artifacts go with which skill is plugin-driven: each setup skill stages
exactly the artifacts its own plugin selects (`artifacts.include` in the plugin
config), so the commons and admin skills never overlap.

Emits, under services/skills/<skill>/assets/artifacts/:
  - <id>.html       — byte-identical to services/artifacts/<id>/view.html
  - manifest.json   — id/name/description/version/isStarred/mcpTools per artifact
                      (no content, so the skill can diff without reading HTML)

Run via `just cowork-setup-assets`; CI-checkable with --check.
"""

import json
import shutil
import sys
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent
ARTIFACTS = REPO / "services" / "artifacts"
SKILLS = REPO / "services" / "skills"
PLUGINS = REPO / "services" / "plugins"

SKILL_TO_PLUGIN = {
    "cowork_setup": "astound-commons",
    "admin_workspace_setup": "astound-admin",
}


def plugin_artifact_ids(plugin: str):
    cfg = yaml.safe_load((PLUGINS / plugin / "config.yaml").read_text())
    artifacts = cfg["plugin"].get("artifacts") or {}
    return artifacts.get("include") or []


def artifact_record(artifact_id: str):
    cfg = yaml.safe_load((ARTIFACTS / artifact_id / "config.yaml").read_text())
    if not cfg.get("enabled", True):
        return None
    return {
        "id": cfg["id"],
        "name": cfg["name"],
        "description": cfg["description"],
        "version": cfg["version"],
        "isStarred": bool(cfg.get("starred", False)),
        "mcpTools": cfg.get("mcp_tools", []),
        "_html": ARTIFACTS / artifact_id / cfg.get("file", "view.html"),
    }


def expected_files(skill: str, plugin: str):
    dest = SKILLS / skill / "assets" / "artifacts"
    records = [r for r in map(artifact_record, plugin_artifact_ids(plugin)) if r]
    if not records:
        sys.exit(f"plugin {plugin} selects no enabled artifacts")
    manifest = json.dumps(
        {"artifacts": [{k: v for k, v in r.items() if k != "_html"} for r in records]},
        indent=2,
        ensure_ascii=False,
    ) + "\n"
    files = {dest / "manifest.json": manifest.encode()}
    for r in records:
        files[dest / f"{r['id']}.html"] = r["_html"].read_bytes()
    return dest, files


def main():
    check = "--check" in sys.argv[1:]
    failed = False
    for skill, plugin in SKILL_TO_PLUGIN.items():
        dest, files = expected_files(skill, plugin)
        if check:
            stale = [
                str(p.relative_to(REPO))
                for p, want in files.items()
                if not p.exists() or p.read_bytes() != want
            ]
            extra = [
                str(p.relative_to(REPO))
                for p in dest.glob("*")
                if p.is_file() and p not in files
            ] if dest.is_dir() else []
            for p in stale:
                print(f"stale or missing: {p}")
            for p in extra:
                print(f"unexpected file: {p}")
            if stale or extra:
                failed = True
            else:
                print(f"{skill}: assets up to date ({len(files) - 1} artifacts)")
            continue
        if dest.is_dir():
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        for p, data in files.items():
            p.write_bytes(data)
        print(f"{skill}: wrote {len(files)} files to {dest.relative_to(REPO)}")
    if failed:
        sys.exit("setup-skill assets out of date — run `just cowork-setup-assets`")


if __name__ == "__main__":
    main()
