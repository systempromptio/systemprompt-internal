---
title: "Create a Conversation and See It Land"
description: "Connect a client, invoke one skill on purpose, then find that conversation under Analysis Conversations and open its transcript. Includes how long to wait and what to check when it does not appear."
author: "systemprompt.io"
slug: "analysis-test-conversation"
keywords: "conversation, skill invocation, bridge, connect code, attribution, revision-verified, hooks, session, transcript"
kind: "guide"
public: true
tags: ["enterprise", "admin", "analytics"]
published_at: "2026-09-16"
updated_at: "2026-09-16"
after_reading_this:
  - "Connect a client to this gateway and invoke a named skill deliberately"
  - "Find the resulting conversation under Analysis › Conversations, filtered to that skill"
  - "Open the transcript for that session"
  - "Diagnose a missing conversation as unbound, no receipt, or snapshot pending"
related_docs:
  - title: "Analysis: Measure, Improve and Publish a Skill"
    url: "/documentation/analysis"
  - title: "Connect Claude Code"
    url: "/documentation/connect-claude-code"
---

# Create a conversation and see it land

**TL;DR:** A conversation only exists because a signed-in person used a skill through a connected client. Connect Claude Code or OpenCode to this gateway, invoke one named skill, wait about a minute, then open `/admin/analysis/skills` filtered to that skill.

## Prerequisites

- A user account on this instance, and its email or user id.
- The gateway's HTTPS address, or `http://localhost:8080` for a local instance.
- At least one skill you can name, visible on `/admin/analysis/skills`. Pick one with a distinctive name.

## Step 1: Connect a client

Pick the route that matches where you are.

**Production or any remote gateway.** Follow [Connect Claude Code](/documentation/connect-claude-code) or [Connect OpenCode](/documentation/connect-opencode). Cowork is a separate client; connecting one does not configure the other.

**A local gateway you are developing against.** Issue a single-use code and run the containerised client:

```bash
systemprompt admin bridge issue-code --user-id you@example.com
just claude <code>
```

Codes are single-use with a ten-minute time to live, so issue one immediately before connecting. The full procedure, including the host-configuring variant, is in [Connect Claude Code](/documentation/connect-claude-code).

**Expected result:** the client starts without prompting for a code again.

## Step 2: Confirm the client is really routed through the gateway

In the connected client, ask any short question. Then open `/admin/history` in the console and confirm a request appears for your user.

A successful sign-in does not prove routing. This step does.

## Step 3: Invoke one skill on purpose

Model access and skill access are separate. Pick a skill by name from `/admin/analysis/skills` and invoke it explicitly rather than hoping the model reaches for it. In Claude Code, ask for the skill by name, or use the slash command your client exposes for it. Let the run finish.

The client's hooks report the invocation to this gateway as it happens: the plugin, the skill, the marketplace, and the services source that shipped it with that source's content hash.

**Expected result:** the skill's instructions are visible in the client's output, not a generic model answer.

## Step 4: Wait for the facts to drain

The feedback fact and snapshot passes run every five seconds, and the inventory refresh every minute. Give it a minute before concluding anything is wrong. Nothing on the Analysis pages is computed live from raw events, so a conversation is invisible until its facts are in a snapshot.

## Step 5: Find the conversation

1. Open `/admin/analysis/skills`.
2. Click the skill's name. Conversations opens filtered to that skill.
3. Set **From (UTC)** and **Until (UTC, exclusive)** so the window covers today, then press **Apply**. The end date is exclusive, so a run from today needs tomorrow's date in **Until**.

Your session should be listed with its consumer, first and last seen times, invocation count, requests, tokens, cost and the latest retained assessment.

## Step 6: Open the transcript

Click the short session id in the first column. That opens the session detail page with the conversation itself. The same session is reachable from `/admin/conversations` and from the skill's row on [Skills](/admin/analysis/skills).

## When it does not appear

Work down this list in order. The three causes are distinct and the fix differs for each.

| Check | Symptom | What it means | Fix |
|---|---|---|---|
| 1. Bound? | The skill reads `unbound` on the Skills table. | The inventory entry is not attached to a managed resource, so nothing can attach metrics to it. | Press **Sync inventory now** and resolve any **Name collisions**. |
| 2. Snapshot? | The skill reads `pending`, or Conversations says facts have not been drained. | The measurement exists but the snapshot has not landed. | Wait a minute and reload. |
| 3. Receipt? | The skill shows invocations in the organisation totals but no attributed use of its own, and Coverage is empty. | Attribution needs verified evidence: a device-authenticated consumer, a session binding and a verified installation receipt. Without the chain the invocation counts in the totals but not in the skill's row. | Confirm the device installed the distributed publication. Receipts are listed under Distribution on that marketplace's [Versions](/admin/analysis/versions) page. |

Two further things worth knowing before you go hunting:

**Attributed is not revision-verified.** Attributed means the hook reported the skill. Revision-verified is stricter: the invocation was proven against a published managed resource that this device verifiably installed. The Skills table's verified share reports the stricter figure, so a conversation can be attributed and still not count as verified.

**Old invocations catch up.** Invocations recorded before a skill identity existed are attributed once the backfill re-normalises them, so an empty table today is not always an empty table tomorrow.

## Troubleshooting

### The client asks for a code on every run

**Symptom:** A repeat run prompts for a connect code.
**Cause:** The stored credential did not validate against this gateway, usually because a different gateway issued it.
**Solution:** `just claude-reset`, then connect with a fresh code.

### The container cannot reach the gateway

**Symptom:** Connection refused from inside the client container.
**Cause:** Inside a container, `localhost` is the container.
**Solution:** `just claude` rewrites the address for you. By hand, use `http://host.docker.internal:8080`.

### The conversation is in History but not in Analysis

**Symptom:** `/admin/history` shows the request, Analysis › Conversations does not.
**Cause:** The request carries no skill identity, so there is nothing to attribute it to.
**Solution:** Re-run invoking the skill explicitly, and confirm the skill is served from a marketplace this user receives.

## Related pages

- [Analysis overview](/documentation/analysis)
- [Measure Which Skills Are Used](/documentation/analysis-measure-skills)
- [Connect Claude Code](/documentation/connect-claude-code)
