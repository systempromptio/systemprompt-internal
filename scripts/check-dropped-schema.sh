#!/usr/bin/env bash
# Gate: a schema file deleted from git leaves no table behind. For every
# `extensions/**/schema/*.sql` (not a migration) that git history shows as
# deleted — the whole history, since the crate deletions that motivated this
# predate the ladder floor — each `CREATE TABLE` it declared must be
# named by a `DROP TABLE` in a tracked migration under extensions/.
#
# Why: 348b8efb removed the knowledge-bank and requirements-workflow crates
# and c791ad5a the session-evaluation extension, none with a drop migration.
# The 2026-09-22 production dump still carried their 19 tables, one
# scheduled_jobs row and a dead `evaluation` migration ledger — schema that
# no code declares, that every fresh install lacks, and that nothing at boot
# reports. The boot-time undeclared-relation check (core SchemaDoctor)
# catches this on a running instance; this gate catches it in the PR.
#
# A migration may drop a table under any name form: `DROP TABLE t`,
# `DROP TABLE IF EXISTS t`, with or without `public.`. A table re-created
# under the same name by a surviving schema file (here or in core) is not
# dead and is skipped.
#
# Known debt, listed by table so nothing else can hide behind it. Each is a
# finding, not an exemption. The list adopted with this gate (odoo_identity,
# email_outbox and the four comms_* tables) was cleared by restoring
# 15_odoo_identity.sql and by migration 084. An entry whose table becomes
# declared or dropped fails the gate as stale.
KNOWN_UNDROPPED=()

set -euo pipefail

cd "$(dirname "$0")/.."

# A table that moved into core is still declared. Core is the sibling
# checkout when the patch is active, else the published crates in the
# cargo registry (what `main` builds against).
core_dirs=()
if [ -d ../systemprompt-core/crates ]; then
    core_dirs+=(../systemprompt-core/crates)
else
    for d in "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/systemprompt-*/; do
        [ -d "$d" ] && core_dirs+=("$d")
    done
fi

declared_now="$({ grep -rhoiE 'CREATE TABLE (IF NOT EXISTS )?[a-z_.]+' extensions "${core_dirs[@]}" --include='*.sql' || true; } \
    | awk '{print tolower($NF)}' | sed 's/^public\.//' | sort -u)"
dropped="$({ grep -rhoiE 'DROP TABLE (IF EXISTS )?[a-z_.]+' extensions --include='*.sql' || true; } \
    | awk '{print tolower($NF)}' | sed 's/^public\.//' | sort -u)"

status=0
known_seen=" "
while IFS=$'\t' read -r commit path; do
    [ -n "$path" ] || continue
    case "$path" in */migrations/*|*/migrations-pending/*|*/seeds/*) continue ;; esac
    tables="$( { git show "${commit}^:${path}" 2>/dev/null || true; } \
        | { grep -oiE 'CREATE TABLE (IF NOT EXISTS )?[a-z_.]+' || true; } \
        | awk '{print tolower($NF)}' | sed 's/^public\.//' | sort -u)"
    for t in $tables; do
        if grep -qx "$t" <<<"$declared_now"; then continue; fi
        if grep -qx "$t" <<<"$dropped"; then continue; fi
        if printf '%s\n' ${KNOWN_UNDROPPED[@]+"${KNOWN_UNDROPPED[@]}"} | grep -qx "$t"; then
            known_seen="$known_seen$t "
            echo "check-dropped-schema: known debt (TODO stage-2): '$t' from $path"
            continue
        fi
        echo "check-dropped-schema: $path (deleted in ${commit:0:8}) declared table '$t' and no migration drops it" >&2
        status=1
    done
done < <(git log --diff-filter=D --name-only --format='%H' HEAD -- 'extensions/**/schema/*.sql' 'extensions/**/schema/**/*.sql' \
    | awk 'NF==1 && /^[0-9a-f]{40}$/ {c=$1; next} NF {print c "\t" $0}')

# A shallow clone has no deletion history to find the debt in, so staleness
# is only judged with full history (gates.yml checks out with fetch-depth 0).
[ "$(git rev-parse --is-shallow-repository)" = "true" ] && known_seen=" ${KNOWN_UNDROPPED[*]} "
for t in ${KNOWN_UNDROPPED[@]+"${KNOWN_UNDROPPED[@]}"}; do
    case "$known_seen" in
        *" $t "*) ;;
        *) echo "check-dropped-schema: stale KNOWN_UNDROPPED entry '$t' (declared or dropped now) — delete it" >&2; status=1 ;;
    esac
done

if [ "$status" -ne 0 ]; then
    echo "check-dropped-schema: add DROP TABLE IF EXISTS … CASCADE to a migration under extensions/web/schema/migrations/" >&2
fi
exit "$status"
