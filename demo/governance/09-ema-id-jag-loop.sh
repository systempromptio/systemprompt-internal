#!/bin/bash
# EMA / ID-JAG END-TO-END LOOP (Okta-independent)
#
# Exercises the full Enterprise-Managed Authorization loop with our own client,
# using a real Odoo id_token as the upstream assertion — no Claude/Okta
# dependency:
#
#   1. ISSUE   POST /token  requested_token_type=id-jag  subject=<SF id_token>
#              → core validates the SF id_token against the trusted issuer and
#                mints a short-lived, client-bound ID-JAG (typ oauth-id-jag+jwt).
#   2. CONSUME POST /token  subject_token_type=id-jag    subject=<ID-JAG>
#              → core verifies typ/aud/client/replay and returns a delegated
#                access token bound to the client owner.
#   3. CALL    GET  the protected MCP with the access token → assert reachable.
#
# This is a guided harness: the SF id_token comes from a real "Sign in with
# Odoo" flow, and the token-exchange client must already be registered.
# Provide them via the environment:
#
#   ODOO_ID_TOKEN      an OIDC id_token from the trusted Odoo issuer
#   EMA_CLIENT_ID      a registered OAuth client id (token-exchange capable)
#   EMA_CLIENT_SECRET  that client's secret
#   EMA_MCP_SERVER     protected MCP to call (default: systemprompt)
#
# When prerequisites are absent the script explains how to obtain them and
# exits 0 (skipped), so it is safe to run in CI without secrets.

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

header "EMA / ID-JAG end-to-end loop"

TOKEN_ENDPOINT="$BASE_URL/api/v1/core/oauth/token"
EMA_MCP_SERVER="${EMA_MCP_SERVER:-systemprompt}"
MCP_ENDPOINT="$BASE_URL/api/v1/mcp/$EMA_MCP_SERVER/mcp"
ID_JAG_TYPE="urn:ietf:params:oauth:token-type:id-jag"
ID_TOKEN_TYPE="urn:ietf:params:oauth:token-type:id_token"
TOKEN_EXCHANGE_GRANT="urn:ietf:params:oauth:grant-type:token-exchange"

if [[ -z "${ODOO_ID_TOKEN:-}" || -z "${EMA_CLIENT_ID:-}" || -z "${EMA_CLIENT_SECRET:-}" ]]; then
  warn "Prerequisites not set — skipping the live EMA loop."
  info "Obtain an Odoo id_token by completing 'Sign in with Odoo'"
  info "and reading the id_token from the OAuth callback, then export:"
  info "  export ODOO_ID_TOKEN=<oidc id_token>"
  info "  export EMA_CLIENT_ID=<registered client id>"
  info "  export EMA_CLIENT_SECRET=<client secret>"
  info "The trusted issuer is configured in profile.yaml (security.trusted_issuers)."
  exit 0
fi

# POST an x-www-form-urlencoded token request; echoes "<body>\n<http_code>".
post_token() {
  curl -s -w $'\n%{http_code}' \
    -X POST "$TOKEN_ENDPOINT" \
    --data-urlencode "grant_type=$TOKEN_EXCHANGE_GRANT" \
    --data-urlencode "client_id=$EMA_CLIENT_ID" \
    --data-urlencode "client_secret=$EMA_CLIENT_SECRET" \
    "$@"
}

split_body() { printf '%s' "$1" | sed '$d'; }
split_code() { printf '%s' "$1" | tail -n1; }

step "1/3  ISSUE — mint an ID-JAG from the Odoo id_token"
issue_resp="$(post_token \
  --data-urlencode "requested_token_type=$ID_JAG_TYPE" \
  --data-urlencode "subject_token=$ODOO_ID_TOKEN" \
  --data-urlencode "subject_token_type=$ID_TOKEN_TYPE")"
issue_code="$(split_code "$issue_resp")"
issue_body="$(split_body "$issue_resp")"

if [[ "$issue_code" != "200" ]]; then
  fail "ISSUE returned HTTP $issue_code: $issue_body"
  exit 1
fi
ID_JAG="$(printf '%s' "$issue_body" | jq -r '.access_token // empty')"
issued_type="$(printf '%s' "$issue_body" | jq -r '.issued_token_type // empty')"
if [[ -z "$ID_JAG" || "$issued_type" != "$ID_JAG_TYPE" ]]; then
  fail "ISSUE did not return an ID-JAG (issued_token_type=$issued_type): $issue_body"
  exit 1
fi
pass "minted ID-JAG (issued_token_type=$issued_type)"

step "2/3  CONSUME — exchange the ID-JAG for a delegated access token"
consume_resp="$(post_token \
  --data-urlencode "subject_token=$ID_JAG" \
  --data-urlencode "subject_token_type=$ID_JAG_TYPE")"
consume_code="$(split_code "$consume_resp")"
consume_body="$(split_body "$consume_resp")"

if [[ "$consume_code" != "200" ]]; then
  fail "CONSUME returned HTTP $consume_code: $consume_body"
  exit 1
fi
ACCESS_TOKEN="$(printf '%s' "$consume_body" | jq -r '.access_token // empty')"
token_type="$(printf '%s' "$consume_body" | jq -r '.token_type // empty')"
if [[ -z "$ACCESS_TOKEN" || "$token_type" != "Bearer" ]]; then
  fail "CONSUME did not return a Bearer access token: $consume_body"
  exit 1
fi
pass "received delegated access token (token_type=$token_type)"

step "3/3  CALL — reach the protected MCP with the delegated token"
mcp_code="$(curl -s -o /dev/null -w '%{http_code}' \
  -X POST "$MCP_ENDPOINT" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","id":1,"method":"tools/list"}')"

if [[ "$mcp_code" == "401" || "$mcp_code" == "403" ]]; then
  fail "MCP rejected the delegated token (HTTP $mcp_code) — audience/scope gate"
  exit 1
fi
pass "protected MCP '$EMA_MCP_SERVER' accepted the delegated token (HTTP $mcp_code)"

divider
pass "EMA loop complete: SF id_token → ID-JAG → access token → MCP"
