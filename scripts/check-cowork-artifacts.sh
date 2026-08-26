#!/usr/bin/env bash
# Gate: the Cowork setup skills' bundled artifact assets must be exactly what
# scripts/sync-cowork-artifacts.py would generate from services/artifacts/.
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/sync-cowork-artifacts.py --check
