---
title: "Conversation History & Search"
description: "Search full AI conversation history — Claude Code sessions and gateway conversations alike — scoped to your identity: yourself always, everything as an admin or auditor."
author: "systemprompt.io"
slug: "enterprise-conversation-history"
keywords: "history, conversations, search, transcripts, audit, manager, fts, gateway"
kind: "guide"
public: true
tags: ["enterprise", "audit", "admin"]
published_at: "2026-08-26"
updated_at: "2026-09-11"
after_reading_this:
  - "Search your own AI conversation history from /admin/history, whichever client produced it"
  - "Open one gateway conversation and read its prompts, responses, and tool calls"
  - "Understand who can see whose history: self, admins and auditors"
  - "Know that snippets are redacted and out-of-scope lookups are refused"
related_docs:
  - title: "Audit Trail, Traceability & Observability"
    url: "/documentation/enterprise-audit-observability"
  - title: "User & Access Management"
    url: "/documentation/enterprise-user-access"
---

# Conversation History & Search

**TL;DR:** `/admin/history` gives every signed-in user full-text search over their own AI conversations — prompts, responses, tools used, timestamps. Who you can search is decided by identity: yourself always, and everything if you hold the `admin` or `auditor` role. Snippets are redacted, and asking for a user outside your scope is refused with 403.

## Two sources, one list

A conversation is recorded in one of two ways, and the history page lists both.

| Source badge | Where it comes from | Recorded by |
|---|---|---|
| Claude Code | A desktop or CLI session on a connected machine | The bridge's session-stop hook, into the transcript store |
| Gateway | A call to `/v1/messages` from any Anthropic-SDK client | The gateway itself, grouped by conversation |

This distinction used to be visible as an absence. A user whose only traffic went through the gateway had no transcript rows at all, so their history page was empty while an admin could see every one of their conversations on the contexts list. Both sources now feed the same list, the same search, and the same paging.

A session that has both a Claude Code transcript and gateway traffic appears twice, once per source. The two records are captured by different mechanisms and hold different fields, so neither is a substitute for the other.

## Searching your history

Open `/admin/history` and search. Claude Code transcripts match by full text over a generated search index; gateway conversations match on the conversation name, the opening prompt, the conversation id, and the model. Results return the conversation with its timestamps, turn count, tokens, and — for gateway conversations — the cost. The same capability is available programmatically at `/admin/api/history/search`.

## Reading one conversation

Selecting a gateway conversation opens `/admin/history/conversations/{id}`: every prompt, response, and tool call in order, with the request telemetry alongside each turn. It is owner-facing, so you do not need the admin role to read your own conversation. Two things are done to the text on the way out:

- The gateway's internal `=== USER PROMPT ===` framing is removed, so you read what you wrote rather than the protocol wrapper around it.
- Every body passes through the credential redactor. The page says how many values were masked rather than altering the text silently.

Operational identifiers — trace ids, request ids, upstream request ids, route matches — are not on this page. An admin gets a link from it to the full context view, which carries them.

Asking for a conversation you do not own returns **404**, the same answer as an id that does not exist. A 403 would confirm the conversation exists and say whose it is not, which would turn the URL into a way to enumerate other people's conversation ids.

## Who sees whose history

| You are | You can search |
|---|---|
| Any signed-in user | Your own conversations, always |
| `admin` or `auditor` role | Everyone — the unrestricted audit view |

Scope is enforced at the query layer for both sources: a request naming a `user_id` outside your scope returns **403**, and the result set can never widen beyond the allowlist your identity resolves to. Project scoping, which narrows the admin-facing entity lists, is deliberately not applied here — it must never hide a user's own conversations from them.

## Redaction carries through

Snippets rendered in search results apply the same display-layer redaction as the rest of the audit surface — values the safety scanners flagged (secrets, PII) stay masked. Reviewing history does not re-expose the sensitive data that was caught on the way through. See [Content Safety & Guardrails](/documentation/enterprise-safety-guardrails).

## Relationship to the audit trail

History search reads the same conversation spine the [audit trail](/documentation/enterprise-audit-observability) records — it is the user- and manager-facing view over data that already exists, not a second store. The knowledge-platform search experience (reusing conversations as knowledge) builds on this same surface — see the [roadmap](/documentation/enterprise-roadmap).

## Verification

Use the commands and UI checks above against the configured instance. Repository maintainers can run `just test-unit` and `just test-integration`; a passing fixture test does not establish that a deployed integration is configured.
