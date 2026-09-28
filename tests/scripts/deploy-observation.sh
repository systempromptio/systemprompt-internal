#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
run_case() (
    source scripts/deploy-observation.sh
    FLY_APP=test HOST=https://example.invalid VERSION=0.57.0 CORE_VERSION=0.57.0 WORKTREE=/retained
    DIGEST_WAIT_SECONDS=15 DIGEST_WAIT_INTERVAL=5 HEALTH_READY_SECONDS=15
    WATCH_SECONDS=30 WATCH_INTERVAL=15
    before_state='sha256:old before' before=sha256:old
    SECONDS=0
    sleep() { SECONDS=$((SECONDS + $1)); }
    log() { :; }
    die() { echo "$*" >&2; exit 1; }
    case "$case_name" in
        unchanged) machine_state() { echo 'sha256:old before'; } ;;
        timestamp_only) machine_state() { echo 'sha256:old after'; } ;;
        digest_only) machine_state() { echo 'sha256:new before'; } ;;
        *) machine_state() { echo 'sha256:new after'; } ;;
    esac
    curl() {
        case "$case_name" in
            readiness_delayed)
                if [ "$SECONDS" -lt 10 ]; then printf '{"status":"starting","version":"0.56.1"}\n200'; return; fi ;;
            wrong_version) printf '{"status":"healthy","version":"0.56.1"}\n200'; return ;;
            outage) if [ "$SECONDS" -ge 15 ]; then printf '{}\n503'; return; fi ;;
            disconnect) if [ "$SECONDS" -ge 15 ]; then return 7; fi ;;
            starting) if [ "$SECONDS" -ge 15 ]; then printf '{"status":"starting","version":"0.57.0"}\n200'; return; fi ;;
        esac
        printf '{ "status": "healthy", "version": "0.57.0" }\n200'
    }
    observe_deploy
    [ "$SECONDS" -ge 30 ]
)
for case_name in healthy readiness_delayed unchanged timestamp_only digest_only wrong_version outage disconnect starting; do
    expected=1
    case "$case_name" in healthy|readiness_delayed) expected=0 ;; esac
    status=0
    run_case >/dev/null 2>&1 || status=$?
    if [ "$status" -ne "$expected" ]; then echo "$case_name: expected $expected, got $status" >&2; exit 1; fi
    echo "$case_name: PASS"
done
