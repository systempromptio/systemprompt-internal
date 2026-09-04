#!/usr/bin/env python3
"""Drop `odoo_identity` rows whose credential no longer works.

`just odoo-sync-local` replaces the local Odoo database wholesale with a copy
of production. `odoo_identity` lives in the *systemprompt* database, which that
restore does not touch — so every stored credential is left pointing at a
res_users/res_users_apikeys row that the restore deleted. The rows still look
linked, and the failure only surfaces later, at the first tool call, as
"Odoo did not accept the stored credential ... open /admin/profile and relink".

That happened for real: a sync on 2026-09-03 stranded the linked account, and
the breakage was not visible until the next Odoo dashboard load, two days on.

Deleting the dead rows is the honest outcome. An unlinked account says so on
the profile page and offers the relink control; a linked-but-dead one claims to
work and fails at the point of use.

Read-only against Odoo; only ever DELETEs from `odoo_identity`.
"""

import json
import pathlib
import sys
import urllib.request

from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
import psycopg2

NONCE_LEN = 12
SECRETS = pathlib.Path(".systemprompt/profiles/local/secrets.json")


def authenticates(url, db, login, key):
    payload = {
        "jsonrpc": "2.0",
        "method": "call",
        "params": {
            "service": "common",
            "method": "authenticate",
            "args": [db, login, key, {}],
        },
        "id": 1,
    }
    request = urllib.request.Request(
        url.rstrip("/") + "/jsonrpc",
        json.dumps(payload).encode(),
        {"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read()).get("result") or None


def main():
    secrets = json.loads(SECRETS.read_text())
    master_key = bytes.fromhex(secrets["encryption_master_key"].strip())
    odoo_url, odoo_db = secrets["odoo_url"], secrets["odoo_db"]

    conn = psycopg2.connect(secrets["database_url"])
    conn.autocommit = True
    cur = conn.cursor()
    cur.execute("SELECT user_id, odoo_login, odoo_uid, odoo_api_key_encrypted FROM odoo_identity")
    rows = cur.fetchall()

    kept = dropped = 0
    for user_id, login, uid, sealed in rows:
        try:
            blob = bytes.fromhex(sealed.strip())
            key = ChaCha20Poly1305(master_key).decrypt(
                blob[:NONCE_LEN], blob[NONCE_LEN:], None
            ).decode()
        except Exception:
            # A credential that cannot be opened cannot be used; it is dead in
            # exactly the way this script exists to clear.
            key = None

        fresh_uid = authenticates(odoo_url, odoo_db, login, key) if key else None
        if fresh_uid is None:
            cur.execute("DELETE FROM odoo_identity WHERE user_id = %s", (user_id,))
            print(f"    unlinked {login} (credential rejected by the restored Odoo)")
            dropped += 1
            continue
        if fresh_uid != uid:
            cur.execute(
                "UPDATE odoo_identity SET odoo_uid = %s, updated_at = CURRENT_TIMESTAMP "
                "WHERE user_id = %s",
                (fresh_uid, user_id),
            )
            print(f"    {login}: uid {uid} -> {fresh_uid} (same credential, new account id)")
        kept += 1

    print(f"    {kept} Odoo identit{'y' if kept == 1 else 'ies'} still valid, {dropped} unlinked")
    return 0


if __name__ == "__main__":
    sys.exit(main())
