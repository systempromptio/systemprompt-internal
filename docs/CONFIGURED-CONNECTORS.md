# Configured personal MCP accounts

Profile authentication is independent of marketplace membership. Active users
can authorize configured personal connectors; resource rules still govern use.
Enabled servers are listed even when `display_in_web` is false. Servers without
personal authorization show “No authorization required”. Saved accounts remain
disconnectable after a server is removed or disabled.

## Generic OAuth

Add the server YAML to the service aggregator's includes:

```yaml
mcp_servers:
  example-tools:
    type: external
    binary: ''
    package: null
    port: 5050
    endpoint: https://mcp.example.com/mcp
    enabled: true
    tool_policy: allow
    display_in_web: false
    oauth:
      required: false
      scopes: [user]
      audience: mcp
      client_id: null
    connector:
      adapter: generic
      scopes: [tools:read]
      authorization_origins:
        - https://login.example.com
      # Omit both for dynamic client registration:
      # client_id_secret: example_mcp_client_id
      # client_secret: example_mcp_client_secret
```

The `oauth` block governs inbound access; `connector` configures outbound OAuth.
Core supplies the credential-broker endpoint automatically when `connector` is
set and `external_auth` is absent. Preserve the existing broker secret.

The resource must publish protected-resource metadata identifying its OAuth
authorization server, whose metadata must advertise PKCE S256. The resource
origin is trusted. Explicitly list any additional discovery, registration,
authorization or token origin. HTTPS is required, including for internal
services with certificates trusted by the host. HTTP redirects are not followed.

Without dynamic registration, configure secret references for a registered
client. Its callback is
`https://YOUR-SERVER/api/public/connectors/example-tools/callback`.
Client secrets never belong in YAML or browser responses.

Generic verification initializes MCP and lists accessible tools. It does not
invent an account identity when no provider identity endpoint exists. Providers
with a specific adapter keep their own verification.

Changing a generic resource or OAuth settings requires reconnection. Refresh
also checks the original issuer and token endpoint. Disconnect removes local
credential access for every enrolled device.

Three optional `connector` keys shape the flow for issuers that need them:

| Key | Effect |
| --- | --- |
| `display_name` | Label on the Connectors row instead of the server id |
| `authorization_params` | Extra query parameters on the authorization request. Only `access_type`, `prompt`, `login_hint` and `hd` are accepted; every parameter the flow itself sets is refused at load |
| `identity: userinfo` | After consent, read `sub` and `email` from the issuer's OIDC `userinfo_endpoint` and show the address on the row. Requires the `openid` scope |

Issuer identifiers are compared as URLs, with a single trailing slash ignored,
so an issuer advertised as `https://idp.example/` in protected-resource metadata
matches `https://idp.example` in its own metadata.

## Failures

Apply migrations with the new binary before adding another provider. Older
binaries cannot manage generic accounts; do not roll back after creating them
without first disconnecting/removing those accounts.

Authorization database failures stop inference with HTTP 503 and a retry hint.
Governance hooks return an explicit deny envelope. Look for
`authorization_unavailable` in logs; the next request evaluates fresh policy
after database access is restored.

## Tool permissions in the clients

Every tool on a managed server is allowed by default in Claude Code and
Claude Desktop/Cowork — the governance chain already judges each call, so the
client's own "Claude wants to use …" prompt adds no control. The bridge writes
`permissions.allow` rules (`mcp__<server>`, plus `mcp__plugin_<plugin>_<server>`
for each plugin that mirrors the server) into Claude Code's managed settings
file when it is writable and `~/.claude/settings.json` otherwise, and a per-tool
`toolPolicy` map into the desktop `managedMcpServers` policy from the tool list
the server reported to the bridge's auth probe (`metadata/mcp-tools.json`).

Every enabled server must declare `tool_policy` on its deployment — `allow`
(the value every server here carries: systemprompt, knowledge-bank, odoo),
`prompt` or `deny`. A server without it is withheld from the signed manifest
and rejected at boot validation:

```yaml
mcp_servers:
  example-tools:
    tool_policy: prompt   # allow | prompt | deny; required
```

The decision travels in the signed manifest (`ManagedMcpServer.tool_policy`
under the `*` key), so a change here reaches every bridge on its next sync.
