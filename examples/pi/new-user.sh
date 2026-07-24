#!/bin/bash
# Register a new (non-admin) user and wire Pi to the gateway as that user.
#
#   examples/pi/new-user.sh [email] [display-name]
#
# What this does:
#   1. Creates the user (default pi-demo@demo.local) with the `user` role.
#   2. Issues a personal access token (sp-live-…) FOR that user via the
#      admin API — the same credential the /admin/devices page self-issues.
#      The gateway accepts PATs directly (x-api-key or Bearer) and resolves
#      the user's roles live from the DB on every request.
#   3. Writes the credential to ~/.config/systemprompt-pi/ so the Pi
#      provider installed by setup.sh picks it up unchanged.
#   4. Smoke-tests POST /v1/messages as the new user.
#
# Portable across macOS and Linux: no grep -P, no head -n -1, no sed -i.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$HERE/../.." && pwd)"
CRED_DIR="$HOME/.config/systemprompt-pi"
EMAIL="${1:-pi-demo@demo.local}"
NAME="${2:-Pi Demo}"

say()  { printf '\033[36m==>\033[0m %s\n' "$*"; }
pass() { printf '\033[32m ok\033[0m %s\n' "$*"; }
die()  { printf '\033[31mERROR:\033[0m %s\n' "$*" >&2; exit 1; }

CLI="$PROJECT_DIR/target/debug/systemprompt"
if [[ -x "$PROJECT_DIR/target/release/systemprompt" && "$PROJECT_DIR/target/release/systemprompt" -nt "$CLI" ]]; then
  CLI="$PROJECT_DIR/target/release/systemprompt"
fi
[[ -x "$CLI" ]] || CLI="$(command -v systemprompt || true)"
[[ -n "$CLI" && -x "$CLI" ]] || die "systemprompt CLI not found. Run: just build"

PROFILE="${PROFILE:-local}"
PROFILE_YAML="$PROJECT_DIR/.systemprompt/profiles/$PROFILE/profile.yaml"
BASE_URL="${BASE_URL:-}"
if [[ -z "$BASE_URL" && -f "$PROFILE_YAML" ]]; then
  BASE_URL=$(grep -E '^[[:space:]]*api_server_url:' "$PROFILE_YAML" | head -1 \
    | sed -E 's/.*api_server_url:[[:space:]]*//; s/[[:space:]]*$//; s/^"//; s/"$//')
fi
BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
BASE_URL="${BASE_URL/localhost/127.0.0.1}"
BASE_URL="${BASE_URL%/}"

# ── 1. Register the user ──
say "Registering user $EMAIL"
"$CLI" admin users create --name "$NAME" --email "$EMAIL" --if-not-exists 2>&1 \
  | grep -viE '^\[profile|already exists' || true
NEW_USER_ID=$("$CLI" admin users search "$EMAIL" 2>/dev/null \
  | grep -oiE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1)
[[ -n "$NEW_USER_ID" ]] || die "could not locate $EMAIL after creation"
pass "User $EMAIL ($NEW_USER_ID), role: user"

# ── 2. Issue a PAT for the user via the admin API ──
say "Issuing a gateway API key for $EMAIL"
ADMIN_TOKEN=$("$CLI" admin session login --token-only 2>/dev/null \
  | grep -oE '[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+' | head -1)
[[ -n "$ADMIN_TOKEN" ]] || die "could not mint an admin session — is the server running? (just start)"

PAT=$(curl -fsS -X POST "$BASE_URL/api/public/admin/users/$NEW_USER_ID/pats" \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d "{\"name\":\"pi-demo $(date -u +%Y%m%dT%H%M%SZ)\"}" | sed -n 's/.*"secret":"\([^"]*\)".*/\1/p')
[[ -n "$PAT" ]] || die "PAT issuance failed (needs the /users/{id}/pats admin endpoint — rebuild + restart?)"
pass "API key issued (${PAT:0:12}…)"

# ── 3. Write Pi credentials ──
mkdir -p "$CRED_DIR"
printf '%s' "$PAT" > "$CRED_DIR/token"
# PATs are not session-bound; the gateway still requires the header, and this
# label groups the demo's audit rows in the dashboard.
printf 'pi-demo-%s' "$NEW_USER_ID" > "$CRED_DIR/session-id"
chmod 600 "$CRED_DIR/token" "$CRED_DIR/session-id"
pass "Pi credentials written to $CRED_DIR"

# ── 4. Smoke test as the new user ──
say "Smoke test: POST $BASE_URL/v1/messages as $EMAIL"
RESP=$(curl -fsS -m 40 -X POST "$BASE_URL/v1/messages" \
  -H "x-api-key: $PAT" -H "x-session-id: pi-demo-$NEW_USER_ID" \
  -H "anthropic-version: 2023-06-01" -H "content-type: application/json" \
  -d '{"model":"claude-sonnet-4-6","max_tokens":32,"messages":[{"role":"user","content":"reply with exactly: pong"}]}') \
  || die "gateway smoke test failed — check: systemprompt infra logs view --level error --since 5m"
printf '%s' "$RESP" | sed -n 's/.*"text":"\([^"]*\)".*/    gateway replied: \1/p'
pass "Gateway accepted the request as $EMAIL"

echo
say "Done. Pi now acts as $EMAIL. Start it with:  pi"
echo "    Manage this user's models at $BASE_URL/admin/models?user_id=$NEW_USER_ID"
echo "    (run examples/pi/setup.sh first if Pi itself is not installed yet)"
