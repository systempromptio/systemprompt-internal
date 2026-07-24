# Pi + Enterprise Demo gateway

Drive [Pi](https://pi.dev) — a minimal, extensible coding-agent CLI — through
the Enterprise Demo governance gateway. Every prompt Pi sends is authenticated,
policy-checked, audited, and cost-tracked, and a single Pi provider entry gives
you models from Anthropic, OpenAI, and Google behind one governed endpoint.

## How it works

Pi supports custom providers via `~/.pi/agent/models.json`. The gateway exposes
an Anthropic-compatible `POST /v1/messages`, so one provider entry with
`"api": "anthropic-messages"` covers everything:

- **Auth** — `Authorization: Bearer <jwt>` minted by
  `systemprompt admin session login --token-only`.
- **Session binding** — the gateway requires an `x-session-id` header equal to
  the JWT's own `session_id` claim; every request is tied to an auditable
  session.
- **Model routing** — the gateway routes by model id glob (`claude-*` →
  Anthropic, `gpt-*` → OpenAI, `gemini-*` → Google). Pi's `/model` picker
  (Ctrl+L) switches between them mid-session; the gateway does the rest.

Secrets never live in Pi config: `models.json` reads the token and session id
from `~/.config/systemprompt-pi/` at request time via Pi's `!command` value
resolution.

## Setup

With the server running (`just start`):

```bash
examples/pi/setup.sh
```

The script installs Pi (npm), mints a gateway token, installs the provider
config and the branded `systemprompt` theme, and smoke-tests `/v1/messages`.
It derives the gateway URL from your active profile
(`.systemprompt/profiles/local/profile.yaml`), so non-default ports just work.

Then:

```bash
pi
```

Pick a model with `/model` — Claude Sonnet 4.6, Claude Opus 4.8, GPT-5 mini,
and Gemini 2.5 Flash are pre-configured.

Tokens expire eventually; re-auth without reinstalling anything:

```bash
examples/pi/setup.sh --refresh
```

## Watch the governance spine

Every Pi turn lands in the same audit tables as every other client:

```bash
systemprompt infra logs request list --limit 10     # one row per /v1/messages hit
systemprompt infra logs audit <request-id> --full   # identity → policy → cost chain
systemprompt analytics costs                        # rollups
```

Try asking the same question on two different models and compare the provider,
token, and cost columns.

The gateway only serves models in its registry (`allow_unlisted_models:
false`). Add a fifth model id that is not registered to `models.json` and the
gateway denies it with a 403 before any upstream call — governance working as
intended.

## Branding

`themes/systemprompt.json` is a Pi theme built from the systemprompt.io
palette (deep navy background, cyan/blue accents). `setup.sh` installs and
activates it; switch themes any time with `/settings`. Pi themes control
colors only — the Pi name and logo are not themeable.

## Files

| File | Purpose |
|------|---------|
| `models.json` | Provider template installed into `~/.pi/agent/models.json` |
| `themes/systemprompt.json` | Branded Pi theme |
| `setup.sh` | Idempotent installer + token mint + smoke test |
