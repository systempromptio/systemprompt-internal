"""Pre-generate the Odoo asset bundles. Idempotent, and safe to run every boot.

Run through `odoo shell` (which injects `env`), not as a standalone script.
The entrypoint does this before the HTTP workers start; `just odoo-assets`
runs it against a live machine.

Why this exists: filestore files are named by the SHA-1 of their content, so
two workers regenerating the *same* bundle concurrently target the *same*
path. If one commits its ir_attachment row while the other rolls back, the
rollback's filestore cleanup deletes the file the committed row points at —
leaving a row with no file, which makes /web/assets/<version>/<bundle> return
500 forever (the "already generated" check passes, then os.stat fails). That
is exactly what happened on 2026-08-19 after the image moved to 18.0-20260803:
the backend rendered unstyled because web.assets_web.min.css 500'd.

Generating in one process before any browser arrives removes the race. It also
repairs the broken state if it is already present, because a row whose file is
missing is regenerated here rather than served.
"""

import os

# Every bundle a first page load pulls: the backend (/odoo), the public
# frontend and its login page, and the report/print bundle.
BUNDLES = [
    "web.assets_web",
    "web.assets_frontend",
    "web.assets_frontend_minimal",
    "web.assets_frontend_lazy",
    "web.assets_web_print",
]

filestore = env["ir.attachment"]._filestore()
failed = []

for name in BUNDLES:
    bundle = env["ir.qweb"]._get_asset_bundle(name, assets_params={})
    for kind in ("css", "js"):
        try:
            attachment = getattr(bundle, kind)()
        except Exception as exc:  # one bad bundle must not block boot
            failed.append(f"{name}.{kind}: {exc}")
            continue
        if not attachment:
            continue
        present = os.path.exists(os.path.join(filestore, attachment.store_fname or ""))
        print(f"[sp-assets] {attachment.url} {'ok' if present else 'MISSING FILE'}")
        if not present:
            failed.append(f"{name}.{kind}: {attachment.url} has no file in the filestore")

# Commit explicitly: `odoo shell` rolls back on exit, which would discard the
# generated rows and delete the files they point at — the very failure mode
# this script exists to prevent.
env.cr.commit()

if failed:
    print("[sp-assets] problems:\n  " + "\n  ".join(failed))
