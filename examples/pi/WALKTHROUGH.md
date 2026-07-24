# Pi demo walkthrough — new user, governed models, full visibility

This walkthrough maps the complete loop: register a new user, act as that
user through the Pi coding agent, then see and govern everything they did
from the dashboard's **Model Selection** page.

Prerequisites: server built and running (`just build`, `just start`), Pi
installed and wired (`examples/pi/setup.sh`), and the per-model gateway
routes added once (`examples/pi/routes.sh`, then restart the server).

## 1. Sign up a new user

```bash
examples/pi/new-user.sh pi-demo@demo.local "Pi Demo"
```

This registers the user with the default `user` role, has the admin API
issue them a personal access token (the same `sp-live-…` credential the
`/admin/devices` page self-issues), writes it to
`~/.config/systemprompt-pi/`, and smoke-tests `/v1/messages` as them.

Browser equivalent: `/admin/register` is the self-signup page
(magic-link + passkey); a logged-in user then issues their own API key
under `/admin/access/devices`. The script automates that path so the demo
is repeatable.

## 2. Log in / credentials

Pi's provider config (`~/.pi/agent/models.json`, installed by `setup.sh`)
reads the credential files at request time, so step 1 already "logged
in" Pi as the new user — no Pi config change needed when the user changes.
The gateway resolves the user's roles live from the database on every
request, so role or rule changes take effect on the next call, not the
next login.

## 3. Drive model requests through Pi

```bash
pi -p --provider systemprompt --model claude-sonnet-4-6 "Reply with exactly one word: pong"
pi -p --provider systemprompt --model claude-opus-4-8   "Reply with exactly one word: pong"
pi -p --provider systemprompt --model gpt-5-mini        "Reply with exactly one word: pong"
pi -p --provider systemprompt --model gemini-2.5-flash  "Reply with exactly one word: pong"
```

Or interactively: `pi`, then `/model` (Ctrl+L) to hop between the four
models mid-session. Every call goes through the governed `/v1/messages`
endpoint — one gateway, three upstream providers.

## 4. See everything in the dashboard

Open **`/admin/models`** (sidebar: Governance → Model Selection) and pick
`pi-demo@demo.local`:

- The model table shows each gateway route, its provider, and whether it
  is enabled for this user.
- The usage panel shows every request the user made: model, provider,
  status, tokens in/out, cost, latency, and per-session denial counts.
- "Open request traces" jumps to `/admin/entities/requests` filtered to
  the user for the full per-request drill-down (identity → policy → cost).

CLI equivalents:

```bash
systemprompt infra logs request list --limit 10
systemprompt infra logs audit <request-id> --full
systemprompt analytics costs
```

## 5. Govern: disable a model, prove the denial

1. On `/admin/models` with the user selected, click **Disable** on
   `gpt-5-mini`. This writes a per-user deny rule; the gateway evaluates
   rules live, so no restart and no new token.
2. Re-run the gpt-5-mini prompt from step 3 — Pi reports a 403
   (`authz denied`). The other three models still answer.
3. The denied attempt lands in the usage table as a failed request and in
   the audit spine (`infra logs audit <id> --full` shows the deny).
4. Click **Enable** — the very next gpt-5-mini request succeeds again.

That is the whole demo: registration → identity-bound requests → live
per-user model governance → complete audit visibility, with Pi as the
untouched third-party client.
