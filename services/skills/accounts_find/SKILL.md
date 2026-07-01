# Find Accounts

Find the accounts you care about without touching a filter or a report. Ask for accounts by name, industry,
region, owner, size, or revenue — in plain English — and get back a clean list with the details that matter.

## Ask me things like

- "Find the Acme account."
- "Show me all accounts in manufacturing."
- "Which accounts are based in California?"
- "List my accounts."
- "Who owns the Globex account?"
- "What are our biggest accounts by revenue?"
- "Show me accounts we added this month."

## Common questions → how I answer them

I read from the **Account** object via the Salesforce MCP (SOQL query). The user never sees the query — I
translate their request, run it, and return a readable list (Name, Industry, Owner, Location, Annual Revenue).

| The user asks | I query |
|---------------|---------|
| An account by name | `SELECT Id, Name, Industry, AnnualRevenue, BillingCity, BillingState, Owner.Name FROM Account WHERE Name LIKE '%Acme%'` — if several match, list them and ask which |
| Accounts in an industry | `WHERE Industry = 'Manufacturing'` — match against the org's real industry picklist; if the wording is fuzzy, confirm which |
| Accounts in a region | `WHERE BillingState = 'CA'` or `BillingCountry = 'United Kingdom'` — use whichever the user meant (city/state/country) |
| My accounts (the signed-in user's) | add `WHERE OwnerId = :currentUser` — resolve via the user's Salesforce identity; if unsure, ask "just yours, or the whole team?" |
| Accounts by owner | look up the user by name, then `WHERE OwnerId = :id` (handle multiple matches by asking which) |
| Biggest accounts by revenue | `WHERE AnnualRevenue != null ORDER BY AnnualRevenue DESC LIMIT 25` |
| Recently created | `WHERE CreatedDate = THIS_MONTH ORDER BY CreatedDate DESC` |

Date literals to prefer: `THIS_MONTH`, `THIS_QUARTER`, `LAST_MONTH`, `THIS_YEAR`, `LAST_N_DAYS:N`. Cap large
result sets (e.g. `LIMIT 50`) and tell the user if there are more.

## Field cheat-sheet (Account)

`Name`, `Industry`, `AnnualRevenue`, `NumberOfEmployees`, `BillingCity`, `BillingState`, `BillingCountry`,
`Owner.Name`, `Type`, `Website`, `Phone`, `LastActivityDate`, `CreatedDate`.

Industries, account types, and any custom fields vary per org — read the actual picklist values rather than
assuming. If a requested field or value doesn't exist in this org, say so.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (SObject query/create/update/delete); refer to them
generically. This skill is **read-only** — to change an account, use the "Update Accounts" skill. Return results
clearly and let the workspace render them; don't format as HTML or build visualisations yourself.
