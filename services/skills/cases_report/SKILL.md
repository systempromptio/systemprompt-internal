# Case Insights

The health of your support queue at a glance. Ask how things are tracking and get a clear rollup — open case
load by priority, owner, and status, what's overdue or at risk, what's been escalated, and how the backlog is
trending — without building a single report.

## Ask me things like

- "How many open cases do we have, by priority?"
- "Break the open queue down by owner."
- "Which cases are overdue or at risk of missing SLA?"
- "Show me everything that's been escalated."
- "How's our backlog trending this month?"
- "How many open cases does each account have?"

## Common questions → how I answer them

I aggregate the **Case** object via the Salesforce MCP. Where the org supports aggregate SOQL I group and count;
otherwise I pull the rows and total them myself. I present the numbers as a plain summary (totals, counts,
per-priority/owner/status breakdown) and let the workspace render it.

| The user asks | How I build it |
|---------------|----------------|
| Open load by priority | `SELECT Priority, COUNT(Id) FROM Case WHERE IsClosed = false GROUP BY Priority` — order by priority severity, show count per priority |
| Open load by owner | `SELECT Owner.Name, COUNT(Id) FROM Case WHERE IsClosed = false GROUP BY Owner.Name ORDER BY COUNT(Id) DESC` |
| Open load by status | `SELECT Status, COUNT(Id) FROM Case WHERE IsClosed = false GROUP BY Status` |
| Overdue / SLA-at-risk | open cases past a target — e.g. `WHERE IsClosed = false AND Priority = 'High' AND CreatedDate < LAST_N_DAYS:2`, or `CreatedDate < LAST_N_DAYS:N` — **flag these explicitly as overdue** |
| Escalated cases | `WHERE IsEscalated = true AND IsClosed = false` — count and list |
| Backlog trend | compare open counts created over recent windows (`LAST_N_DAYS:7` vs prior) or closed-vs-created to show whether the queue is growing or shrinking |
| Cases per account | `SELECT Account.Name, COUNT(Id) FROM Case WHERE IsClosed = false GROUP BY Account.Name ORDER BY COUNT(Id) DESC` |

Always state the scope you used — whose cases, which period, which priority — so the numbers are unambiguous.
Call out overdue and escalated cases explicitly rather than burying them in a total. If aggregate queries aren't
permitted for the user, fall back to fetching rows (with a sensible `LIMIT`) and counting, and say if the result
was capped.

## Field cheat-sheet (Case)

`Status`, `Priority`, `Origin`, `IsClosed`, `IsEscalated`, `Owner.Name`, `Account.Name`, `Contact.Name`,
`CreatedDate`, `ClosedDate`, `LastModifiedDate`.

Statuses, priorities, and SLA/escalation rules are configured per org — read the real values. If the org uses
custom SLA or milestone fields, prefer those and mention it.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset; refer to them generically. This skill is
**read-only**. Return the summary as clear structured text/numbers — how it's charted or laid out is the
workspace's job, so don't emit HTML or build visualisations yourself.
