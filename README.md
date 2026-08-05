<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="storage/files/images/logo-white.svg">
  <source media="(prefers-color-scheme: light)" srcset="storage/files/images/logo.svg">
  <img src="storage/files/images/logo.svg" alt="Astound Digital" width="380">
</picture>

# Transformation That Endures.

The Astound Digital branded AI governance platform. One self-hosted binary governs inference, auditing, and every tool call across your AI fleet. Any agent, any model, any provider.

[![Built on systemprompt-core](https://img.shields.io/badge/built%20on-systemprompt--core-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core)
[![Template · MIT](https://img.shields.io/badge/template-MIT-16a34a?style=flat-square)](LICENSE)
[![Core · BSL--1.1](https://img.shields.io/badge/core-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94+-f97316?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL 18](https://img.shields.io/badge/postgres-18-336791?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

[**astounddigital.com**](https://astounddigital.com) · [**Platform documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

## Setup

From nothing to Claude Code running against your own governed gateway.

### 1. Prerequisites

| Requirement | Why | Install |
|---|---|---|
| **Rust 1.94+** | Compiles the binary | [rustup.rs](https://rustup.rs/) |
| **Docker** | Runs PostgreSQL 18 | [docs.docker.com](https://docs.docker.com/get-docker/) |
| **`just`** | Task runner — every command below | [just.systems](https://just.systems/) |
| **`jq`, `yq`** | Used by the setup scripts | `apt install jq yq` / `brew install jq yq` |
| **An AI API key** | Anthropic, OpenAI, or Gemini — one is enough | Provider dashboard |

Ports `8080` (HTTP) and `5432` (Postgres) must be free.

### 2. Clone

```bash
git clone https://github.com/systempromptio/systemprompt-astound
cd systemprompt-astound
```

One repository is enough: the workspace builds against the published `systemprompt` release from crates.io. The `[patch.crates-io]` blocks in `Cargo.toml` and `tests/Cargo.toml` are commented out and are only uncommented — in lockstep, since `[patch]` applies per-workspace — to work against a sibling checkout of core while a change to it is unreleased.

### 3. Set up and start

```bash
just setup-local     # builds the binary, writes .systemprompt/profiles/local/,
                     # starts Docker Postgres, runs the publish pipeline
just start           # governance + agents + MCP + admin on :8080
```

`setup-local` prompts for your provider and its key. Non-interactive instead — the first key given becomes the default provider:

```bash
just setup-local <anthropic_key> [openai_key] [gemini_key]
```

Second clone on the same host? Override the ports: `just setup-local <key> "" "" 8081 5433`.

### 4. Connect Claude Code

Open **http://localhost:8080/admin/profile** and copy your connect code, then:

```bash
just claude <code>
```

That builds the client, starts a clean container, signs it in with your code, and drops you straight into Claude Code. Your host config is never touched — no installer runs on your machine, nothing in `~/.claude` or `~/.config` is modified.

Every request from that session lands in your audit table with user, session, trace, tokens, and cost.

The container is `astound-claude`, and its home lives in the `astound-claude-home` Docker volume so a second run reuses the stored credential instead of burning a code. Sign out with `just claude-reset`.

### Verifying the flow from a clean state

Worth running after any change to the connect path. The failure mode is silent: a machine that already holds a valid credential skips the sign-in entirely and still reports success.

Clone into a new directory — no profile, no sibling checkout — on ports of its own so it cannot collide with a gateway you already run:

```bash
git clone https://github.com/systempromptio/systemprompt-astound fresh && cd fresh
just setup-local <provider-key> "" "" 8081 5436
just build
just start
```

The fresh database has no users. Make one, promote it, and issue it a code — the same one-shot code the profile page mints, so this needs no browser:

```bash
systemprompt admin users create --email you@example.com --if-not-exists
systemprompt admin users role promote you@example.com
systemprompt admin bridge issue-code --user-id you@example.com
```

Clear any stored session before connecting. **This is the step that makes the test meaningful** — skip it and a surviving credential from an earlier run will carry the test:

```bash
just claude-reset
just claude <code> http://localhost:8081
```

Read the output rather than trusting the exit code. It must say **"signing in with the supplied code"**. If it says *"already signed in — reusing the stored PAT"*, the sign-in never ran and the result proves nothing.

`astound-bridge doctor` runs at the end with a pass/fail line per check. One warning is expected: the hook-token check reports no OAuth client yet, because provisioning is lazy — it happens on the first plugin hook request, not during sync.

### 5. Day-to-day

```bash
just build            # debug build (--release for release)
just preflight        # the CI gate: static → lint → tests → coverage
just publish          # rebuild templates, CSS, JS, assets
systemprompt --help   # discover the CLI
```

---

## License

**This template** is [MIT](LICENSE). Fork it, modify it, use it however you like.

**[systemprompt-core](https://github.com/systempromptio/systemprompt-core)** is [BSL-1.1](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE): free for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

---

<div align="center">

[![systemprompt.io](https://img.shields.io/badge/systemprompt.io-2b6cb0?style=for-the-badge)](https://systemprompt.io) &nbsp; [![Core](https://img.shields.io/badge/systemprompt--core-2b6cb0?style=for-the-badge)](https://github.com/systempromptio/systemprompt-core) &nbsp; [![Documentation](https://img.shields.io/badge/documentation-16a34a?style=for-the-badge)](https://systemprompt.io/documentation/) &nbsp; [![Guides](https://img.shields.io/badge/guides-f97316?style=for-the-badge)](https://systemprompt.io/guides) &nbsp; [![Discord](https://img.shields.io/badge/discord-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/wkAbSuPWpr)

<sub>Own how your organization uses AI. Every interaction governed and provable.</sub>

</div>
