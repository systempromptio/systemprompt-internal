# Find Leads

Find the prospects you care about without touching a filter or a report. Ask for leads by status, source,
owner, company, or industry — in plain English — and get back a clean list with the details that matter.

## Ask me things like

- "Show me the new leads that came in this week."
- "Which leads are still open and haven't been converted?"
- "List my leads in the Working status."
- "What leads came from the webinar last month?"
- "Show me leads at companies in the software industry."
- "Who are my hottest leads right now?"

## Common questions → how I answer them

I read from the **Lead** object via the Salesforce MCP (SOQL query). The user never sees the query — I
translate their request, run it, and return a readable list (Name, Company, Title, Status, Source, Owner).

| The user asks | I query |
|---------------|---------|
| New leads this week | `SELECT Id, FirstName, LastName, Company, Title, Status, LeadSource, Owner.Name, CreatedDate FROM Lead WHERE CreatedDate = THIS_WEEK ORDER BY CreatedDate DESC` (or `CreatedDate = LAST_N_DAYS:7`) |
| Open / unconverted leads | `WHERE IsConverted = false ORDER BY CreatedDate DESC` |
| My leads (the signed-in user's) | add `AND OwnerId = :currentUser` — resolve via the user's Salesforce identity; if unsure, ask "just yours, or the whole team?" |
| Leads in a named status | `WHERE Status = 'Working'` — match against the org's real status picklist; if the name is fuzzy, confirm which |
| Leads from a source | `WHERE LeadSource = 'Web'` — match the org's source picklist values |
| Leads by company or industry | `WHERE Company LIKE '%Acme%'` or `WHERE Industry = 'Software'` |
| Hottest / best-rated leads | `WHERE Rating = 'Hot' AND IsConverted = false ORDER BY CreatedDate DESC` |

Date literals to prefer: `THIS_WEEK`, `THIS_MONTH`, `LAST_WEEK`, `LAST_MONTH`, `THIS_YEAR`, `LAST_N_DAYS:N`.
Exclude converted leads with `IsConverted = false` unless the user asks about converted history. Cap large
result sets (e.g. `LIMIT 50`) and tell the user if there are more.

## Field cheat-sheet (Lead)

`FirstName`, `LastName`, `Name`, `Company`, `Title`, `Email`, `Phone`, `Status`, `LeadSource`, `Industry`,
`Rating`, `Owner.Name`, `IsConverted`, `CreatedDate`, `LastActivityDate`.

Status, source, rating, and industry picklists — plus any custom fields — vary per org, so read the actual
values rather than assuming. If a requested field or status doesn't exist in this org, say so.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (SObject query/create/update/delete); refer to them
generically. This skill is **read-only** — to change a lead, use the "Update Leads" skill. Return results
clearly and let the workspace render them; don't format as HTML or build visualisations yourself.
