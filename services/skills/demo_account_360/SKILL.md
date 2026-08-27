# Demo 2 — Account 360 Brief

The "service / account agent" use case: everything the business knows about one lead, assembled live from
typed tool results across **two** MCP servers. Salesforce sells this as a Data-Cloud-backed 360; here it is
four read-only tool calls, each executed as the signed-in user, each audited, each priced.

## When to Use

- Step 2 of the enterprise demo, immediately after `demo_lead_triage` (use the hottest lead it flagged).
- Any "brief me on <account/lead>" request where the full demo framing helps.

## Script

Pick the target lead (from step 1, or ask). Then orchestrate — all reads, safe to run in parallel:

| Call | Server | Provides |
|------|--------|----------|
| `crm_lead_get` | odoo | The record itself: stage, revenue, owner, contact |
| `note_list` on the lead | odoo | Chatter — what has been said, by whom, when |
| `attachment_list` on the lead | odoo | Documents on the record |
| `search_project_context` with the lead/company name | knowledge-bank | Related transcripts and documents outside the CRM |

Present the brief in this order, short: **Snapshot** (one paragraph from the record) → **Conversation**
(newest chatter first, summarized) → **Documents** (names + what they are) → **Wider context** (knowledge
bank hits, or one line saying there are none) → **Suggested next step** (exactly one, tied to the data —
it seeds step 3).

Optionally open the **Business Overview** artifact to show the same data plane feeding a standing
dashboard.

## Cost readback

```bash
systemprompt infra logs trace list --limit 10       # four tool calls, two servers, one session
systemprompt infra logs request list --limit 5      # tokens + cost for the synthesis
```

State: N tool calls across 2 servers, total cost $X — every row carries the user's identity and a
trace id.

## Talking Points

- Cross-system orchestration is skill prose, not integration code: the typed contracts do the work.
- The knowledge bank is the "Data Cloud" analogue — except it is self-hosted and queried per-user.
- Empty sections are stated, never padded: the brief is only what the tools returned this run.

## Rules

- Name every record with its Odoo id; the follow-up step mutates them and must be unambiguous.
- No memory of previous briefs as fact; if a call fails, say which one and show the error.
- Hand off: end by offering step 3, `demo_followup_orchestrator`, on your suggested next step.
