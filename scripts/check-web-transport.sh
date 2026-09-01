#!/usr/bin/env bash
# A server-rendered page reads the pool it is served from. Nothing else.
#
# The admin dashboard once read through three transports at once: the Profile
# pane went straight to Postgres, the MCP-servers card probed the loopback
# proxy, and the Activity card shelled out to the CLI through an MCP tool. That
# last one made a page render depend on the active profile, on a tenant session,
# and on the working directory the MCP server happened to start in — so the card
# reported "Not connected" on a machine where everything was, in fact, connected.
#
# The transports are collapsed. This gate stops the third one growing back:
# code under extensions/web/** may not spawn a subprocess, and may not build an
# HTTP client aimed at this instance's own gateway. Reaching a genuinely
# external service is still fine; calling ourselves over the network is not.
#
# Exemption: `// lint-ok: web-transport` on or directly above the line, with a
# reason. Doctrine that cannot be exempted gets worked around instead.
set -uo pipefail

WEB_DIR="${WEB_DIR:-extensions/web}"

[ -d "$WEB_DIR" ] || { echo "check-web-transport: no $WEB_DIR - nothing to check"; exit 0; }

python3 - "$WEB_DIR" <<'PY'
import re, sys, pathlib

root = pathlib.Path(sys.argv[1])

# (pattern, why it is banned)
BANS = [
    (re.compile(r'\bstd::process::Command\b|\bprocess::Command::new\b|\bCommand::new\('),
     "spawns a subprocess; a page render must not depend on a CLI invocation"),
    (re.compile(r'\btokio::process\b'),
     "spawns a subprocess; a page render must not depend on a CLI invocation"),
    (re.compile(r'mcp__systemprompt__|\bsystemprompt_mcp_agent\b'),
     "routes page data through the MCP CLI tool; read the pool instead"),
    # Narrow on purpose: naming the instance's own URL is usually *display*
    # (a link in a page, robots.txt, a "run this" hint), which is fine. What is
    # banned is the SSR read path building an HTTP client at all — those
    # handlers have the pool in hand and must use it.
    (re.compile(r'\breqwest::(Client|get|post)\b|\bhyper::client\b|\bClient::builder\('),
     "builds an HTTP client on the SSR read path; read the pool it is served from"),
]

# The last ban applies only to the server-rendered read path. Jobs and content
# extenders legitimately fetch genuinely external services.
HTTP_SCOPE = ("admin/src/handlers/", "admin/src/services/", "admin/src/repositories/")

EXEMPT = re.compile(r'lint-ok:\s*web-transport')
violations = []

for path in sorted(root.rglob("*.rs")):
    if "/target/" in str(path):
        continue
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        continue
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("*"):
            continue
        for pat, why in BANS:
            if not pat.search(line):
                continue
            if "HTTP client" in why and not any(sc in str(path) for sc in HTTP_SCOPE):
                continue
            # Exemption on this line or in the comment block directly above it.
            if EXEMPT.search(line):
                continue
            j, exempt = i - 1, False
            while j >= 0:
                above = lines[j].strip()
                if not above:
                    break
                if not (above.startswith("//") or above.startswith("#[")):
                    break
                if EXEMPT.search(above):
                    exempt = True
                    break
                j -= 1
            if exempt:
                continue
            violations.append(f"{path}:{i + 1}: {why}\n    {stripped}")

if violations:
    print("check-web-transport: a web surface reached outside its own process\n")
    for v in violations:
        print(f"  {v}")
    print(
        "\nA server-rendered page reads the pool it is served from, through a typed\n"
        "repository. If you genuinely need an exemption, annotate the line with\n"
        "`// lint-ok: web-transport` and say why."
    )
    sys.exit(1)

print("check-web-transport: ok")
PY
