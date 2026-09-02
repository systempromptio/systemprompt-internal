#!/bin/bash
# Runs ON the Fly Odoo machine (see `just odoo-sync-local`). The database
# password is a Fly secret, readable only from the machine's own odoo.conf,
# so the dump has to be taken here and fetched afterwards. Read-only.
set -euo pipefail
CONF=/tmp/odoo.conf
DB=$(sed -n 's/^db_name *= *//p' "$CONF" | tr -d ' ')
HOST=$(sed -n 's/^db_host *= *//p' "$CONF" | tr -d ' ')
USER=$(sed -n 's/^db_user *= *//p' "$CONF" | tr -d ' ')
PGPASSWORD=$(sed -n 's/^db_password *= *//p' "$CONF" | tr -d ' ')
export PGPASSWORD
[ -n "$DB" ] || DB="$USER"
pg_dump -h "$HOST" -U "$USER" -d "$DB" -Fc -f /tmp/sync.dump
tar czf /tmp/sync-filestore.tar.gz -C /var/lib/odoo "filestore/$DB"
ls -l /tmp/sync.dump /tmp/sync-filestore.tar.gz
