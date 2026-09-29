# Required secrets

Everything an operator must provide for Claude Code and OpenCode to work end to
end. Each provider entry in
[`services/ai/providers.yaml`](../../services/ai/providers.yaml) names the
secret it reads its credential from; the profile secret store supplies the
value per environment. The catalog is shipped in the image and is identical in
every environment — only the secrets differ.

## The secrets

| secret | provider entry | credential | unlocks |
|---|---|---|---|
| `anthropic` | `anthropic` | Anthropic API key | every `claude-*` model (`claude-opus-5-5`, `claude-opus-5`, `claude-sonnet-5`, `claude-fable-5-1`, `claude-fable-5`, `claude-haiku-4-5`, and the Claude 4.x models kept for pinned clients) |
| `cerebras` | `cerebras` | Cerebras API key | `gpt-oss-120b`, and every `gpt-*` / `gemini-*` id, which `gateway.yaml` routes to it via `upstream_model` |
| `openai` | `openai` | OpenAI API key | nothing today: the entry is in the catalog for pricing, but no route dispatches to it |

One secret is the minimum: `anthropic` alone gives Claude Code and OpenCode a
working default, since `gateway.yaml` sets `default_provider: anthropic` and
`default_model: "claude-sonnet-5[1m]"` (Claude Code's 1M-context form of
`claude-sonnet-5`; other hosts get the bare id). `cerebras` adds the open-weight
model and the `gpt-*` / `gemini-*` ids served by it.

Beyond the providers, core refuses to start without `encryption_master_key`
(64 hex characters). The container entrypoint mints one on first boot when the
secrets file lacks it; on a deployment where secrets come from a vault, supply
it and keep it — stored connector grants cannot be decrypted with a different
key.

A model whose provider has no secret is still advertised on `/v1/models` — that
endpoint filters by API surface, not by credential — and fails at dispatch with
`Gateway API key secret '<name>' not configured`. Advertise only what you have
credentialed, or expect that error in the audit trail.

## Setting a secret

```bash
systemprompt admin config secret set <name> <value>
```

`set` is the only subcommand. It writes into the secrets file of the **active
profile**, so switch profiles first when targeting a deployed environment
(`systemprompt admin session switch production`). Infrastructure secrets —
database URLs, the at-rest pepper, the signing seed — are refused by this
command and are provisioned out of band.

The secret store is loaded once at process start
(`SecretsBootstrap::get`), so a newly set secret takes effect on the next server
restart, not immediately.

## Verifying

**The catalog, unauthenticated.** `/v1/models` carries no auth layer, so this
needs no credential and proves only that the catalog loaded and the surface
filter works:

```bash
curl -sS localhost:8080/v1/models | jq -r '.data[].id'
```

**A real dispatch, which is what actually proves a secret.** `/v1/messages`
needs a PAT *and* a minted session; an arbitrary `x-session-id` is rejected
rather than created on demand.

```bash
PAT=$(systemprompt admin users api-key issue --user <user-id> --name scratch \
        | grep -o 'sp-live-[^ ]*')
SID=$(curl -sS -X POST localhost:8080/api/public/gateway/sessions \
        -H "Authorization: Bearer $PAT" -H 'content-type: application/json' \
        -d '{}' | jq -r .session_id)

curl -sS -X POST localhost:8080/v1/messages \
  -H "Authorization: Bearer $PAT" -H "x-session-id: $SID" \
  -H 'content-type: application/json' -d '{
    "model":"claude-haiku-4-5","max_tokens":64,
    "messages":[{"role":"user","content":"ping"}]}' | jq '{stop:.stop_reason, usage}'
```

Substitute the model id for whichever secret you are checking (`gpt-oss-120b`
for `cerebras`). A credential problem surfaces as a `PreAudit` dispatch error
naming the secret; a rejected key surfaces as the provider's own `401` relayed
through.

Every call above lands a row with the user, the model, the tokens and the cost:

```bash
systemprompt infra logs request list --limit 5
```

## How a developer reaches these models

Nothing changes on the developer's machine. The bridge install points Claude
Code at the gateway and enables gateway model discovery, so every credentialed
model appears in the `/model` picker. OpenCode reads the same catalog. A request
names the catalog id, the gateway routes it by the patterns in
[`services/ai/gateway.yaml`](../../services/ai/gateway.yaml), rewrites it to the
upstream name where `upstream_model` differs, and audits the call.
