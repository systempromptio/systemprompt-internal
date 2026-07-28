#!/usr/bin/env bash
# Enforce the front-end coding standards on storage/files/{js,css}.
#
# The structural half of the gate (manifest/disk agreement, template
# references, line limits, token uniqueness) lives in the Rust test
# extensions/web/site/tests/asset_manifest.rs and runs under cargo test.
# This script covers the textual rules that are cheapest to express as
# greps: banned constructs in JS, centralisation of fetch/event
# registration, and CSS hygiene.
#
# Exemption: list a `path:rule` pair (one per line, `#` comments) in
# scripts/frontend-standards-exemptions.txt. Reserve it for cases with a
# documented reason, never as a way to mute a fixable violation.
set -uo pipefail

JS_DIR="${JS_DIR:-storage/files/js}"
CSS_DIR="${CSS_DIR:-storage/files/css}"
EXEMPT_FILE="${EXEMPT_FILE:-scripts/frontend-standards-exemptions.txt}"

fail=0

exempt() {
  local file="$1" rule="$2"
  [ -f "$EXEMPT_FILE" ] && grep -qxF "${file}:${rule}" "$EXEMPT_FILE"
}

report() {
  local rule="$1" match="$2"
  local file="${match%%:*}"
  if ! exempt "$file" "$rule"; then
    echo "FAIL[$rule] $match"
    fail=1
  fi
}

check_js() {
  local rule="$1" pattern="$2" exclude="${3:-^$}"
  while IFS= read -r match; do
    [ -n "$match" ] && report "$rule" "$match"
  done < <(grep -rnE "$pattern" "$JS_DIR" --include='*.js' | grep -vE "$exclude" | grep -v 'admin-bundle')
}

check_css() {
  local rule="$1" pattern="$2" exclude="${3:-^$}"
  while IFS= read -r match; do
    [ -n "$match" ] && report "$rule" "$match"
  done < <(grep -rnE "$pattern" "$CSS_DIR" --include='*.css' | grep -vE "$exclude" | grep -v 'admin-bundle')
}

check_js var '\bvar ' ''
check_js loose-equality '[^=!<>]==[^=]' 'null'
check_js eval '\beval\(' ''
check_js default-export 'export default' ''
check_js alert-confirm-prompt '\b(alert|confirm|prompt)\(' 'showConfirm|showPrompt|\.confirm\(|\.prompt\('
check_js console 'console\.(log|debug|info|warn|error)' ''
check_js comments '^\s*(//|/\*)' ''
check_js raw-fetch '[^a-zA-Z.]fetch\(' 'services/api\.js|^'"$JS_DIR"'/(analytics|homepage|blog-list|docs|mobile-menu)|site/'
check_js document-click-listener "document\.addEventListener\('click'" 'services/events\.js|^'"$JS_DIR"'/(analytics|homepage|blog-list|docs|mobile-menu)|site/'
check_js empty-catch '\.catch\(\(\) => \{\}\)|\.catch\(\(\) => \(\{\}\)\)' ''
check_js json-clone 'JSON\.parse\(JSON\.stringify' ''
check_js legacy-dom '\.appendChild\(|\.removeChild\(' 'content\.cloneNode'

check_css important '!important' 'prefers-reduced-motion|animation-duration|animation-iteration-count|transition-duration|scroll-behavior|blog-print\.css'
check_css at-import '@import' ''
check_css id-selector '^#[a-z]' ''
check_css token-fallback 'var\(--sp-[a-z0-9-]+,' 'var\(--sp-fill|var\(--sp-progress|var\(--sp-section-color|var\(--sp-xp-pct'
check_css css-comments '/\*' 'core/fonts\.css'

if [ "$fail" -ne 0 ]; then
  echo "check-frontend-standards: violations found"
  exit 1
fi
echo "check-frontend-standards: OK"
