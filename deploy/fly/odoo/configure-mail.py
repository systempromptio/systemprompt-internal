"""Configure outgoing mail on the Odoo companion app. Idempotent.

Run through `odoo shell` (which injects `env`), not as a standalone script:

    just odoo-mail-config

Credentials come from the machine environment (Fly secrets set by
`just odoo-mail-secrets`), never from this file and never from the volume.

Outbound only. There is deliberately no `fetchmail.server` here: Odoo 18 CE
polls INBOX with no folder selector, so an inbound server pointed at
brain@systemprompt.io would race the knowledge extension's `email_ingestion`
job (services/scheduler/config.yaml) for the same unseen messages. Inbound
needs its own mailbox first.
"""

import os

SERVER_NAME = "Resend (systemprompt.io)"


def require(name):
    value = os.environ.get(name)
    if not value:
        raise SystemExit(f"missing {name} in the machine environment — run `just odoo-mail-secrets` first")
    return value


smtp_host = require("SMTP_HOST")
smtp_port = int(require("SMTP_PORT"))
smtp_user = require("SMTP_USER")
smtp_password = require("SMTP_PASSWORD")
# Bare domain, no display name: from_filter and the alias domain are matched
# against it, so "systemprompt.io <hello@systemprompt.io>" would not match.
mail_domain = os.environ.get("MAIL_DOMAIN", "systemprompt.io")
company_email = os.environ.get("MAIL_COMPANY_EMAIL", f"hello@{mail_domain}")
base_url = os.environ.get("ODOO_BASE_URL", "https://odoo.systemprompt.io")

# --- Outgoing server ------------------------------------------------------
values = {
    "name": SERVER_NAME,
    "smtp_host": smtp_host,
    "smtp_port": smtp_port,
    "smtp_encryption": "starttls",
    "smtp_authentication": "login",
    "smtp_user": smtp_user,
    "smtp_pass": smtp_password,
    # Relay-level allowlist. Odoo rewrites the envelope sender to the alias
    # domain's default_from whenever the author is outside this filter, which
    # is what stops it forging a customer address and failing SPF at Resend.
    "from_filter": mail_domain,
    "sequence": 10,
    "active": True,
}
server = env["ir.mail_server"].search([("name", "=", SERVER_NAME)], limit=1)
if server:
    server.write(values)
    print(f"updated ir.mail_server {server.id} ({SERVER_NAME})")
else:
    server = env["ir.mail_server"].create(values)
    print(f"created ir.mail_server {server.id} ({SERVER_NAME})")

# --- Alias domain: bounce, catchall, and the default envelope sender ------
alias_values = {
    "name": mail_domain,
    "bounce_alias": "bounce",
    "catchall_alias": "catchall",
    "default_from": "odoo",
}
alias_domain = env["mail.alias.domain"].search([("name", "=", mail_domain)], limit=1)
if alias_domain:
    alias_domain.write(alias_values)
    print(f"updated mail.alias.domain {alias_domain.id} ({mail_domain})")
else:
    alias_domain = env["mail.alias.domain"].create(alias_values)
    print(f"created mail.alias.domain {alias_domain.id} ({mail_domain})")

companies = env["res.company"].search([])
companies.write({"alias_domain_id": alias_domain.id})
# The stock database ships info@yourcompany.com, which leaks into templates.
for company in companies:
    if not company.email or company.email.endswith("@yourcompany.com"):
        company.email = company_email
print(f"bound {len(companies)} company(ies) to {mail_domain}, email={companies[0].email}")

# --- Base URL: every link in every outgoing mail is built from this -------
params = env["ir.config_parameter"].sudo()
params.set_param("web.base.url", base_url)
# Without the freeze, the next admin login over any other hostname silently
# rewrites the parameter and links start pointing at fly.dev again.
params.set_param("web.base.url.freeze", "True")
print(f"web.base.url={params.get_param('web.base.url')} (frozen)")

env.cr.commit()

# --- Verify against the live relay ---------------------------------------
# Raises UserError if the relay refuses the connection or the credentials.
server.test_smtp_connection()
print(f"SMTP connection to {smtp_host}:{smtp_port} OK")
