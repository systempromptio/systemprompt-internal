#!/bin/bash
# SMOKE TEST — seed, prove both reports render, prove the customer copy leaks
# nothing internal, then restore the database.
#
# The leak check is the point of this script. The customer report is a document
# that leaves the building, so "it does not carry our cost or margin" has to be
# asserted against the rendered HTML rather than trusted to code review.
#
# Repeatable by construction: it unseeds on the way out, including when an
# assertion fails.

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

# The shared demo token is a hook/plugin-audience service token. The admin
# console authenticates against the `api` audience, so a token that works for
# the governance endpoints is redirected to the login page here — mint a real
# admin session instead.
TOKEN=$("$CLI" admin session login --token-only --profile "$PROFILE" 2>/dev/null | tail -1)
if [[ -z "$TOKEN" ]]; then
  echo "ERROR: could not mint an admin session token" >&2
  exit 1
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
FAILURES=0

cleanup() {
  bash "$HERE/02-unseed-report-data.sh" --quiet || true
}
trap cleanup EXIT

header "REPORTS SMOKE TEST" "Seed, render, assert, restore"

bash "$HERE/01-seed-report-data.sh" > /dev/null
pass "Seed applied"
echo ""

subheader "Render the internal P&L"
INTERNAL=$(curl -s -w '\n%{http_code}' -H "Authorization: Bearer $TOKEN" \
  "$BASE_URL/admin/reports/internal")
INTERNAL_CODE="${INTERNAL##*$'\n'}"
INTERNAL_BODY="${INTERNAL%$'\n'*}"

if [[ "$INTERNAL_CODE" == "200" ]]; then
  pass "GET /admin/reports/internal -> 200"
else
  fail "GET /admin/reports/internal -> $INTERNAL_CODE"
  FAILURES=$((FAILURES + 1))
fi

if grep -q "Gross margin" <<<"$INTERNAL_BODY"; then
  pass "Internal report states a margin"
else
  fail "Internal report is missing its margin figure"
  FAILURES=$((FAILURES + 1))
fi
echo ""

subheader "Render the customer report"
CUSTOMER=$(curl -s -w '\n%{http_code}' -H "Authorization: Bearer $TOKEN" \
  "$BASE_URL/admin/reports/customer?org=astound-digital")
CUSTOMER_CODE="${CUSTOMER##*$'\n'}"
CUSTOMER_BODY="${CUSTOMER%$'\n'*}"

if [[ "$CUSTOMER_CODE" == "200" ]]; then
  pass "GET /admin/reports/customer -> 200"
else
  fail "GET /admin/reports/customer -> $CUSTOMER_CODE"
  FAILURES=$((FAILURES + 1))
fi

for expected in "Astound Digital" "By department" "By model" "By user"; do
  if grep -qF "$expected" <<<"$CUSTOMER_BODY"; then
    pass "Customer report has '$expected'"
  else
    fail "Customer report is missing '$expected'"
    FAILURES=$((FAILURES + 1))
  fi
done
echo ""

subheader "Assert the customer report leaks nothing internal"
# `Margin`, `Provider cost`, and the per-seat unit economics are the internal
# report's vocabulary. None may appear on the customer's copy.
#
# Only meaningful against a real report: a redirect body contains none of these
# words either, and a vacuous pass here is worse than no check at all.
if [[ "$CUSTOMER_CODE" != "200" ]]; then
  fail "Cannot check for leaks — the customer report did not render"
  FAILURES=$((FAILURES + 1))
fi
for banned in "Margin" "Provider cost" "Per seat" "Cost per 1M"; do
  if grep -qF "$banned" <<<"$CUSTOMER_BODY"; then
    fail "LEAK: customer report contains '$banned'"
    FAILURES=$((FAILURES + 1))
  else
    pass "No '$banned' on the customer report"
  fi
done
echo ""

subheader "Restore"
bash "$HERE/02-unseed-report-data.sh"
trap - EXIT
echo ""

if [[ $FAILURES -eq 0 ]]; then
  pass "SMOKE TEST PASSED"
else
  fail "SMOKE TEST FAILED — $FAILURES assertion(s)"
  exit 1
fi
