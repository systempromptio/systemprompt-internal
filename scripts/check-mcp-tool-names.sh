#!/usr/bin/env bash
# Every `mcp__<server>__<tool>` string in the repo must name a tool that the
# server actually registers.
#
# The wire name is free text everywhere it is written — artifact YAML, artifact
# HTML, skill prose — and until this gate existed only the *server* half was
# checked (validate-services.sh, step 8). The tool half was checked by nothing,
# so `mcp__knowledge_bank__list_documents` (underscore, where the server id is
# `knowledge-bank`) shipped and failed in front of a user as "No such tool
# available". Nothing normalises a hyphen, so that is not a near miss.
#
# The catalog is built from the servers' own `pub const TOOL_*` declarations
# rather than a list kept here, so it cannot drift from what registers. A
# server that names a tool with an inline literal is invisible to that scan —
# which is why extensions/mcp/systemprompt declares TOOL_SYSTEMPROMPT.
#
# As with check-file-size.sh, the gate proves it can fail before it is allowed
# to report success: a run first plants known-bad text and asserts the scan
# trips on it.
set -uo pipefail
cd "$(dirname "$0")/.."

python3 - <<'EOF'
import difflib
import pathlib
import re
import sys

import yaml

root = pathlib.Path(".")
errors = []

# A trailing `*` means the text is a glob standing in for a family of names
# (`mcp__systemprompt__delete_*` in prose), not a name to resolve.
# The lookahead rejects the whole match rather than letting it backtrack to a
# shorter valid-looking name: `mcp__systemprompt__delete_*` must not resolve as
# `delete`.
WIRE = re.compile(r"mcp__([A-Za-z0-9_-]+)__([A-Za-z0-9_]+)(?![A-Za-z0-9_*])")

# Why: a `"tool_name"` field in a PreToolUse payload is the governance surface,
# and the governance demos deliberately post names no server registers — that
# a name is about to be *denied* is the thing being shown. Only the invocation
# surface has to resolve.
HOOK_PAYLOAD = re.compile(r'"tool_name"\s*:\s*"(mcp__[A-Za-z0-9_-]+__[A-Za-z0-9_]+)"')
CONST = re.compile(r'^pub const (SERVER_NAME|TOOL_[A-Z0-9_]+): &str = "([^"]+)";', re.M)

# Why: these servers are provided by the host application (Claude Desktop and
# its Cowork mode), not by this repo, so they can never appear in a catalog
# built from our own `pub const TOOL_*` declarations. Naming one is
# documentation about the host's own tools, not the misspelling of one of ours
# that this gate exists to catch. The exemption is the server half only, and
# the list is explicit: any other undeclared server still fails.
HOST_PROVIDED = {"workspace"}


def load(path):
    try:
        return yaml.safe_load(path.read_text()) or {}
    except yaml.YAMLError as e:
        errors.append(f"{path}: unparseable YAML: {e}")
        return {}


# --- the declared servers -------------------------------------------------
# Ids are the mapping keys under `mcp_servers:`, verbatim — hyphens included.
declared = {}
for path in sorted(root.glob("services/mcp/*.yaml")):
    for sid, doc in (load(path).get("mcp_servers") or {}).items():
        declared[sid] = doc if isinstance(doc, dict) else {}


def enabled(doc):
    return doc.get("enabled", True) is not False


# Why: an artifact that no enabled plugin ships is shelved, not broken. Its
# names must still be spelled correctly — it gets unshelved eventually, and a
# typo that lands then is the same bug arriving later — but pointing at a
# disabled server is only a defect if something actually delivers it. This is
# the same shipped-ness test validate-services.sh applies.
shipped = set()
for pl_path in root.glob("services/plugins/*/config.yaml"):
    doc = load(pl_path)
    plugins = doc.get("plugins") or ({"_": doc["plugin"]} if "plugin" in doc else {})
    for plugin in plugins.values():
        if not isinstance(plugin, dict) or not enabled(plugin):
            continue
        arts = (plugin.get("artifacts") or {}).get("include") or []
        shipped.update(arts)


# --- the catalog, read out of each server's own source --------------------
catalog = {}
for crate in sorted(root.glob("extensions/mcp/*/src")):
    names, server = set(), None
    for rs in crate.rglob("*.rs"):
        for const, value in CONST.findall(rs.read_text(errors="replace")):
            if const == "SERVER_NAME":
                server = value
            else:
                names.add(value)
    if server and names:
        catalog[server] = names

missing_catalog = sorted(
    sid for sid, doc in declared.items()
    if enabled(doc) and doc.get("type") == "internal" and sid not in catalog
)
for sid in missing_catalog:
    errors.append(
        f"services/mcp: internal server '{sid}' exposes no extractable tool names — "
        f"declare `pub const SERVER_NAME` and `pub const TOOL_*: &str` in its crate so "
        f"this gate can vouch for names that reference it"
    )


def suggest(server, tool):
    """The nearest real name, so a typo gate does not just move the guessing."""
    pool = [f"mcp__{server}__{t}" for t in catalog.get(server, ())]
    pool += [f"mcp__{s}__{t}" for s in catalog for t in catalog[s] if t == tool]
    hit = difflib.get_close_matches(f"mcp__{server}__{tool}", sorted(set(pool)), n=1, cutoff=0.5)
    return f" — did you mean '{hit[0]}'?" if hit else ""


def scan_text(label, text, check_enabled=True):
    """Check every wire name in one file. Pure, so the self-check can call it."""
    found = []
    exempt = set(HOOK_PAYLOAD.findall(text))
    for server, tool in WIRE.findall(text):
        if f"mcp__{server}__{tool}" in exempt:
            continue
        found.append((server, tool))
        if server in HOST_PROVIDED:
            continue
        if server not in declared:
            errors.append(
                f"{label}: 'mcp__{server}__{tool}' names unknown mcp_server "
                f"'{server}'{suggest(server, tool)}"
            )
        elif check_enabled and not enabled(declared[server]):
            errors.append(
                f"{label}: 'mcp__{server}__{tool}' names disabled mcp_server '{server}' — "
                f"enable the server or shelve the artifact"
            )
        elif server in catalog and tool not in catalog[server]:
            errors.append(
                f"{label}: '{server}' registers no tool '{tool}'{suggest(server, tool)}"
            )
    return found


# --- surface 1 + 2: artifacts (config.yaml allowlist, view.html calls) ----
for cfg in sorted(root.glob("services/artifacts/*/config.yaml")):
    ships = cfg.parent.name in shipped
    allow = {t for _, t in scan_text(str(cfg), cfg.read_text(errors="replace"), ships)}
    view = cfg.parent / "view.html"
    if not view.exists():
        continue
    for server, tool in scan_text(str(view), view.read_text(errors="replace"), ships):
        # Why: a bundle that calls a tool its own allowlist omits is rejected
        # at fetch time with "not in this artifact's mcp_tools allowlist" — a
        # runtime failure that reads as a broken dashboard.
        if tool not in allow:
            errors.append(
                f"{view}: calls 'mcp__{server}__{tool}' but {cfg.name} does not list it "
                f"under mcp_tools — add it to the allowlist"
            )

# --- surface 3: skills ----------------------------------------------------
skill_docs = sorted(root.glob("services/skills/*/SKILL.md")) + sorted(
    root.glob("storage/files/plugins/*/skills/*/SKILL.md")
)
all_tools = {t for tools in catalog.values() for t in tools}

for doc in skill_docs:
    text = doc.read_text(errors="replace")
    # Skills name tools in prose across many hosts; whether a given server is
    # switched on here is not a property of the prose.
    scan_text(str(doc), text, check_enabled=False)
    # Bare names, checked only in probe rows — a markdown table cell that
    # invokes a tool with an argument object. Narrow on purpose: the same
    # identifiers appear as prose all over the skill tree, and a gate that
    # cries wolf on prose gets muted.
    for line in text.splitlines():
        if not (line.startswith("|") and "with `{" in line):
            continue
        for name in re.findall(r"`([a-z][a-z0-9_]{4,})`", line):
            # The server-id cell of a probe row is not a tool name.
            if name in declared:
                continue
            if name not in all_tools:
                hit = difflib.get_close_matches(name, sorted(all_tools), n=1, cutoff=0.6)
                tail = f" — did you mean '{hit[0]}'?" if hit else ""
                errors.append(f"{doc}: probe row names unknown tool '{name}'{tail}")


# --- prove the scan can fail ---------------------------------------------
before = len(errors)
scan_text("<self-check>", "mcp__knowledge-bank__no_such_tool mcp__no-such-server__x")
scan_text("<self-check>", '"tool_name":"mcp__knowledge-bank__exempt_payload"')
scan_text("<self-check>", "mcp__knowledge-bank__list_* is a glob, not a name")
scan_text("<self-check>", "mcp__workspace__bash is host-provided, not ours")
if len(errors) - before != 2:
    print(
        "error: check-mcp-tool-names self-check failed — the scan did not flag "
        "a bad tool name and an unknown server.",
        file=sys.stderr,
    )
    sys.exit(1)
del errors[before:]

if errors:
    print("mcp tool name validation FAILED:")
    for e in errors:
        print(f"  {e}")
    sys.exit(1)
print(f"mcp tool names OK ({len(catalog)} servers, {len(all_tools)} tools)")
EOF
