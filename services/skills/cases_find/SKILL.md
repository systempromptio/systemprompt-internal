# Find Cases

Find the support cases you care about without touching a filter or a report. Ask for cases by status, priority,
account, owner, or when they came in — in plain English — and get back a clean list with the details that matter.

## Ask me things like

- "Show me all the open cases."
- "What high-priority cases are still open?"
- "List the cases on the Acme account."
- "Which cases are assigned to me?"
- "Show me everything that's been escalated."
- "What cases came in this week?"

## Common questions → how I answer them

I read from the **Case** object via the Salesforce MCP (SOQL query). The user never sees the query — I translate
their request, run it, and return a readable list (CaseNumber, Subject, Status, Priority, Account, Owner).

| The user asks | I query |
|---------------|---------|
| Open cases | `SELECT Id, CaseNumber, Subject, Status, Priority, Origin, Account.Name, Contact.Name, Owner.Name FROM Case WHERE IsClosed = false ORDER BY CreatedDate DESC` |
| My cases (the signed-in user's) | add `AND OwnerId = :currentUser` — resolve via the user's Salesforce identity; if unsure, ask "just yours, or the whole team?" |
| Cases by priority | `WHERE Priority = 'High' AND IsClosed = false ORDER BY CreatedDate` — match against the org's real priority picklist |
| Escalated cases | `WHERE IsEscalated = true AND IsClosed = false` |
| Cases on a named account | look up the Account by name first, then `WHERE AccountId = :id` (handle multiple matches by asking which) |
| Cases in a named status | `WHERE Status = 'Working'` — match against the org's real status picklist; if the name is fuzzy, confirm which status |
| Cases created recently | `WHERE CreatedDate = LAST_N_DAYS:7 ORDER BY CreatedDate DESC` |
| High-priority open cases | `WHERE IsClosed = false AND Priority IN ('High') ORDER BY CreatedDate` (add escalated with `OR IsEscalated = true` if asked) |

Date literals to prefer: `THIS_WEEK`, `TODAY`, `THIS_MONTH`, `LAST_MONTH`, `THIS_YEAR`, `LAST_N_DAYS:N`.
Always exclude closed cases with `IsClosed = false` unless the user asks about closed/resolved history. Cap large
result sets (e.g. `LIMIT 50`) and tell the user if there are more.

## Field cheat-sheet (Case)

`CaseNumber`, `Subject`, `Status`, `Priority`, `Origin`, `Account.Name`, `Contact.Name`, `Owner.Name`,
`IsClosed`, `IsEscalated`, `CreatedDate`, `ClosedDate`, `LastModifiedDate`.

Status values, priorities, origins, and any custom fields vary per org — read the actual picklist values rather
than assuming. If a requested field or status doesn't exist in this org, say so.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (SObject query/create/update/delete); refer to them
generically. This skill is **read-only** — to change a case, use the "Update Cases" skill. Return results
clearly and let the workspace render them; don't format as HTML or build visualisations yourself.
