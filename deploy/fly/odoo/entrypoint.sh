#!/bin/bash
set -euo pipefail

# Render config from env (Fly secrets + [env]) — /tmp so the volume never
# holds credentials and a stale rendered file can't shadow new secrets.
envsubst < /etc/odoo/odoo.conf.template > /tmp/odoo.conf

# First boot: initialize the database (installs base). Guarded by a marker
# on the persistent volume so redeploys skip it.
MARKER=/var/lib/odoo/.sp-db-initialized
if [ ! -f "$MARKER" ]; then
  echo "[sp-entrypoint] first boot: initializing database ${ODOO_DB_NAME}"
  odoo -c /tmp/odoo.conf -d "${ODOO_DB_NAME}" -i base --without-demo=all --stop-after-init
  touch "$MARKER"
fi

# Pre-generate the asset bundles in this single process, before the HTTP
# workers can race each other over them. Non-fatal: a failure here costs
# styling on the first page load, not the boot. See pregenerate-assets.py.
echo "[sp-entrypoint] pre-generating asset bundles"
odoo shell -c /tmp/odoo.conf -d "${ODOO_DB_NAME}" --no-http --log-level=warn \
  < /usr/local/lib/sp-pregenerate-assets.py || \
  echo "[sp-entrypoint] WARNING: asset pre-generation failed; continuing"

exec odoo -c /tmp/odoo.conf
