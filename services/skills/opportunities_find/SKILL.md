# Find Deals

Find the deals you care about without touching a filter or a report. Ask for opportunities by stage, size,
close date, owner, or account — in plain English — and get back a clean list with the details that matter.

## Ask me things like

- "Show me my open deals closing this quarter."
- "What deals are over $50k and still open?"
- "List everything in the negotiation stage."
- "Which deals on the Acme account are still open?"
- "What did we close last month?"
- "Show me deals that haven't moved in 30 days."

## Common questions → how I answer them

I read from the **Opportunity** object via the Salesforce MCP (SOQL query). The user never sees the query — I
translate their request, run it, and return a readable list (Name, Account, Stage, Amount, Close Date, Owner).

| The user asks | I query |
|---------------|---------|
| Open deals closing this quarter | `SELECT Id, Name, Account.Name, StageName, Amount, CloseDate, Owner.Name FROM Opportunity WHERE IsClosed = false AND CloseDate = THIS_QUARTER ORDER BY CloseDate` |
| My deals (the signed-in user's) | add `AND OwnerId = :currentUser` — resolve via the user's Salesforce identity; if unsure, ask "just yours, or the whole team?" |
| Deals over an amount | `WHERE Amount >= 50000 AND IsClosed = false ORDER BY Amount DESC` |
| Deals in a named stage | `WHERE StageName = 'Negotiation/Review'` — match against the org's real stage picklist; if the name is fuzzy, confirm which stage |
| Deals on a named account | look up the Account by name first, then `WHERE AccountId = :id` (handle multiple matches by asking which) |
| Won/lost in a period | `WHERE StageName = 'Closed Won' AND CloseDate = LAST_MONTH` (or `IsWon = true`) |
| Stalled / not moving | `WHERE IsClosed = false AND LastModifiedDate < LAST_N_DAYS:30 ORDER BY LastModifiedDate` |

Date literals to prefer: `THIS_QUARTER`, `THIS_MONTH`, `NEXT_QUARTER`, `LAST_MONTH`, `THIS_YEAR`, `LAST_N_DAYS:N`.
Always exclude closed deals with `IsClosed = false` unless the user asks about won/lost history. Cap large
result sets (e.g. `LIMIT 50`) and tell the user if there are more.

## Field cheat-sheet (Opportunity)

`Name`, `Account.Name`, `StageName`, `Amount`, `CloseDate`, `Probability`, `ForecastCategory`, `Owner.Name`,
`IsClosed`, `IsWon`, `Type`, `LeadSource`, `NextStep`, `LastModifiedDate`, `CreatedDate`.

Stage names, record types, and any custom fields vary per org — read the actual picklist values rather than
assuming. If a requested field or stage doesn't exist in this org, say so.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (SObject query/create/update/delete); refer to them
generically. This skill is **read-only** — to change a deal, use the "Update Deals" skill. Return results
clearly and let the workspace render them; don't format as HTML or build visualisations yourself.
