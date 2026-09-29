# Centralized MCP connections

External MCP servers are reached through the gateway, not configured on each
desktop. Each person links their own account on the server — on
`/admin/connectors` (or `/admin/profile` on older builds) — and the bridge shows
the same server-owned connection model in its Profile view; **Manage on server**
opens that page. Provider credentials are never placed in a marketplace bundle or
desktop config.

This page is the operating model. The YAML shape of a connector and the OAuth
rules it follows are in [configured connectors](../CONFIGURED-CONNECTORS.md). The
Odoo connector, which uses per-user API keys rather than OAuth, is documented at
`/documentation/odoo`.

## Local development

Edit core in the sibling `../systemprompt-core` checkout when a connector change
needs one. Both server Cargo workspaces and the bridge resolve through that path
while the `[patch.crates-io]` blocks are active; `bridge/CORE_REF` pins the
committed core revision for CI.

## Provisioning

Connector enablement is `mcp_servers.<id>.enabled` in the server's
`services/mcp/<id>.yaml`, included by `services/config/config.yaml`. The account
page, bridge manifest, and credential accessor read the same loaded services
configuration. Restart after changes. There is no separate enablement flag in
secrets or environment variables.

Store the settings below in the active profile's secret store or as equivalent
uppercase environment variables. Never put credentials in service YAML. Reuse the
instance's existing `encryption_master_key`; do not rotate it for a connector.
`oauth_at_rest_pepper` hashes OAuth identifiers and cannot replace the encryption
key used to recover provider tokens.

| Setting | Value |
| --- | --- |
| `encryption_master_key` | Persistent 32-byte key as 64 hexadecimal characters; required to store OAuth grants. Preserve it across restarts. |
| `mcp_credential_broker_secret` | Random server-only secret of at least 32 bytes, shared by core and the account accessor in the same instance. |
| `<provider>_mcp_client_id`, `<provider>_mcp_client_secret` | A registered OAuth client, when the provider does not support dynamic client registration. Named by `client_id_secret` / `client_secret` in the connector block. |

Set `server.api_external_url` to the deployment's HTTPS origin and provision its
credentials independently; local grants are never copied into production.
Register one callback per connector under that origin:

- `https://YOUR-SERVER/api/public/connectors/<server-id>/callback`

A provider whose organization restricts callback domains or source IPs must allow
the server's callback domain and outbound IP.

Registry definitions stay enabled because enabled marketplace plugins must
resolve their server references. The manifest filter independently requires
provider configuration, the person's entitlement in
`services/access-control/rules.yaml`, and a verified connection before publishing
a connector to them. A registered server is not proof of a working provider
connection.

## Acceptance checks

1. Use two entitled users with different provider permissions and one user
   without the entitlement.
2. Link accounts on the server. Check the same identities and states in the
   bridge.
3. Sync a clean Claude Code installation. Verify the connectors with `/mcp` and
   make a read-only request against each provider, without a provider CLI login.
4. Enroll another machine as the same person. Sync and repeat without consent.
5. Disconnect an account. Requests must fail immediately; both account views
   refresh within 15 seconds while visible, with a 30-second bridge background
   refresh. Manifest sync follows availability changes.
6. Exercise expired grants, provider outages, callback replay and account
   switches, concurrent refresh, and denial for the unentitled user. An outage
   preserves the grant.
7. Check manifests and local config for the absence of provider tokens. Check
   audit identity and trace ids for tool calls and governance denials.

There is no automatic switch to a shared application identity or to direct
desktop access.

## Verification and deployment

Failed account verification preserves encrypted grants for retry while blocking
connector use until verification succeeds. Tenant identity must match the
provider's authenticated resource list.

Validate bridge synchronization, ordinary-user access, multiple devices, refresh,
disconnect and provider outages for the deployment being released. Verify MCP
session continuity across replicas before running multiple application
instances. A single successful account check does not establish these
properties.
