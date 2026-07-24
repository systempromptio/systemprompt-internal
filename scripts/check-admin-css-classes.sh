#!/usr/bin/env bash
# Check that every CSS class the admin templates reference has a rule to match.
#
# Admin pages are server-rendered from Handlebars templates; the CSS ships
# separately as a bundle. Nothing links the two, so a renamed or deleted rule
# leaves the markup referencing a class that no longer styles anything, and the
# page quietly degrades. This gate reads every `class="..."` in the templates
# and partials and fails if a token has no matching `.token` rule anywhere in
# the admin or core CSS sources.
#
# Handlebars is stripped before tokens are split: `{{...}}` expressions are
# removed, and any token still holding `{{`/`}}`, starting with a non-letter,
# shorter than three characters, or ending in `-` (the stump of a stripped
# dynamic modifier) is ignored — those are dynamic or too generic to check.
#
# Exemption: list a class (one per line, `#` comments) in
# scripts/admin-css-class-exemptions.txt. Reserve it for classes toggled or
# generated at runtime by JS, never for a rule that is simply missing.
set -uo pipefail

TPL_DIR="${TPL_DIR:-storage/files/admin/templates}"
PARTIAL_DIR="${PARTIAL_DIR:-storage/files/admin/partials}"
CSS_DIRS="${CSS_DIRS:-storage/files/css/admin storage/files/css/core}"
EXEMPT_FILE="${EXEMPT_FILE:-scripts/admin-css-class-exemptions.txt}"

[ -d "$TPL_DIR" ] || { echo "check-admin-css-classes: no $TPL_DIR - nothing to check"; exit 0; }

python3 - "$TPL_DIR" "$PARTIAL_DIR" "$EXEMPT_FILE" $CSS_DIRS <<'PY'
import re, sys, pathlib

tpl_dir, partial_dir, exempt_file = sys.argv[1], sys.argv[2], sys.argv[3]
css_dirs = sys.argv[4:]

# Runtime-toggled / JS-generated classes never carry a static rule.
exempt = set()
p = pathlib.Path(exempt_file)
if p.exists():
    for line in p.read_text().splitlines():
        line = line.split('#', 1)[0].strip()
        if line:
            exempt.add(line)

# Build the CSS corpus and a membership test: a class `foo` is satisfied when
# `.foo` appears not immediately followed by another class-name character.
css = []
for d in css_dirs:
    for f in sorted(pathlib.Path(d).rglob('*.css')):
        css.append(f.read_text())
css = '\n'.join(css)

_cache = {}
def has_rule(cls):
    if cls not in _cache:
        _cache[cls] = re.search(r'\.' + re.escape(cls) + r'(?![\w-])', css) is not None
    return _cache[cls]

CLASS_ATTR = re.compile(r'class\s*=\s*"([^"]*)"')

def classes_in(text):
    """Class tokens from a template, with handlebars expressions removed."""
    # Strip every `{{...}}` from the whole file BEFORE finding attributes: an
    # expression can contain a `"` (e.g. `{{#if (eq x "active")}}`) that would
    # otherwise terminate the class="..." match early and spill junk tokens.
    # Braces do not nest here, so a non-greedy match is exact.
    text = re.sub(r'\{\{.*?\}\}', ' ', text, flags=re.S)
    found = set()
    for m in CLASS_ATTR.finditer(text):
        raw = m.group(1)
        for tok in raw.split():
            if '{{' in tok or '}}' in tok:
                continue
            if len(tok) < 3:
                continue
            if not tok[0].isalpha():
                continue
            # A token ending in `-` is the stump of a dynamic modifier whose
            # suffix was a stripped `{{...}}` (e.g. `cc-bp-item--{{status}}`);
            # no real class name ends in a hyphen.
            if tok.endswith('-'):
                continue
            found.add(tok)
    return found

templates = sorted(pathlib.Path(tpl_dir).glob('*.hbs'))
templates += sorted(pathlib.Path(partial_dir).rglob('*.hbs'))

violations = []
for path in templates:
    missing = sorted(
        c for c in classes_in(path.read_text())
        if c not in exempt and not has_rule(c)
    )
    if missing:
        violations.append((path, missing))

if violations:
    total = sum(len(m) for _, m in violations)
    print("check-admin-css-classes: class(es) with no matching CSS rule:", file=sys.stderr)
    for path, missing in violations:
        print(f"  {path}", file=sys.stderr)
        for c in missing:
            print(f"    .{c}", file=sys.stderr)
    print("", file=sys.stderr)
    print(f"{total} missing class(es) across {len(violations)} template(s).", file=sys.stderr)
    print("Add the rule to storage/files/css/, or if the class is toggled by JS,", file=sys.stderr)
    print(f"list it in {exempt_file} with a reason.", file=sys.stderr)
    sys.exit(1)

print("check-admin-css-classes: OK (every referenced class has a matching rule)")
PY
