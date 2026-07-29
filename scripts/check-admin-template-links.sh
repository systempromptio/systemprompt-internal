#!/usr/bin/env bash
# Check that every internal admin link in the templates points at a real route.
#
# Admin pages are server-rendered and cross-link by hard-coded `/admin/...`
# hrefs. When a route is renamed or removed the href is left dangling, and the
# only signal is a 404 the next person to click it discovers. This gate reads
# every static `href="/admin/..."` in the templates and partials and fails if
# it does not match a route registered in the admin route modules.
#
# The route corpus is every `.route("...")` literal under
# extensions/web/admin/src/routes/*.rs. Those routers are nested under an
# `/admin` prefix, so a literal `/governance/policies` serves
# `/admin/governance/policies`. A `{param}` segment matches any single path
# segment. Query strings and fragments are ignored, and any href containing
# `{{` is dynamic and skipped.
#
# Exemption: list a path (one per line, `#` comments) in
# scripts/admin-link-exemptions.txt for intentional targets that are not SSR
# routes (external-ish paths, redirect stubs). Never exempt a real dead link.
set -uo pipefail

TPL_DIR="${TPL_DIR:-storage/files/admin/templates}"
PARTIAL_DIR="${PARTIAL_DIR:-storage/files/admin/partials}"
ROUTE_DIR="${ROUTE_DIR:-extensions/web/admin/src/routes}"
EXEMPT_FILE="${EXEMPT_FILE:-scripts/admin-link-exemptions.txt}"

[ -d "$TPL_DIR" ] || { echo "check-admin-template-links: no $TPL_DIR - nothing to check"; exit 0; }

python3 - "$TPL_DIR" "$PARTIAL_DIR" "$ROUTE_DIR" "$EXEMPT_FILE" <<'PY'
import re, sys, pathlib

tpl_dir, partial_dir, route_dir, exempt_file = sys.argv[1:5]

exempt = set()
p = pathlib.Path(exempt_file)
if p.exists():
    for line in p.read_text().splitlines():
        line = line.split('#', 1)[0].strip()
        if line:
            exempt.add(line)

# Route corpus: every `.route("literal")` under the admin route modules, served
# under the `/admin` nest prefix. Each becomes a regex where `{param}` matches
# one segment. A handful of alias/redirect targets exist without their own
# `.route(...)` literal, so they are seeded explicitly.
route_literals = set()
for f in sorted(pathlib.Path(route_dir).glob('*.rs')):
    for m in re.finditer(r'\.route\(\s*"([^"]*)"', f.read_text()):
        route_literals.add(m.group(1))

# Aliases registered outside a plain `.route(...)` literal (redirects, nests).
route_literals |= {'/user', '/access-control'}

patterns = []
for lit in route_literals:
    served = '/admin' + ('' if lit == '/' else lit)
    rx = re.sub(r'\{[^}]+\}', r'[^/]+', re.escape(served).replace(r'\{', '{').replace(r'\}', '}'))
    patterns.append(re.compile('^' + rx + '$'))

def is_route(path):
    return any(rx.match(path) for rx in patterns)

HREF = re.compile(r'href\s*=\s*"([^"]*)"')

def links_in(text):
    for m in HREF.finditer(text):
        href = m.group(1)
        if not href.startswith('/admin'):
            continue
        if '{{' in href:
            continue
        # Drop query string and fragment.
        href = href.split('?', 1)[0].split('#', 1)[0]
        yield href

templates = sorted(pathlib.Path(tpl_dir).glob('*.hbs'))
templates += sorted(pathlib.Path(partial_dir).rglob('*.hbs'))

violations = []
for path in templates:
    dead = sorted({h for h in links_in(path.read_text())
                   if h not in exempt and not is_route(h)})
    if dead:
        violations.append((path, dead))

if violations:
    total = sum(len(d) for _, d in violations)
    print("check-admin-template-links: href(s) with no matching admin route:", file=sys.stderr)
    for path, dead in violations:
        print(f"  {path}", file=sys.stderr)
        for h in dead:
            print(f"    {h}", file=sys.stderr)
    print("", file=sys.stderr)
    print(f"{total} dead link(s) across {len(violations)} template(s).", file=sys.stderr)
    print("Register the route in extensions/web/admin/src/routes/, fix the href,", file=sys.stderr)
    print(f"or if the target is intentionally not an SSR route list it in {exempt_file}.", file=sys.stderr)
    sys.exit(1)

print("check-admin-template-links: OK (every admin link resolves to a route)")
PY
