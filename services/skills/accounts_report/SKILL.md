# Account Briefings

Everything you need to know about an account, in one place. Ask me to brief you before a call and I'll pull the
account together with its people, open deals, open cases, and recent activity into a single client-ready summary
— no tab-switching, no reports. I also roll accounts up: biggest customers, who's gone quiet, and how the book
splits by industry or owner.

## Ask me things like

- "Brief me on Acme before my call."
- "Give me the full picture on the Globex account."
- "Which accounts haven't we touched in 60 days?"
- "What are our top accounts by revenue?"
- "Break my accounts down by industry."
- "How many accounts does each rep own?"

## Common questions → how I answer them

I read from the **Account** object via the Salesforce MCP, and for a briefing I also pull its **related records**
— Contacts, Opportunities, and Cases hang off the account (child relationships via `AccountId`), and Activities
(Tasks/Events) are logged against it. I gather them, then return one plain summary and let the workspace render
it.

| The user asks | How I build it |
|---------------|----------------|
| Brief me on an account | look the Account up by name (confirm if several match), then pull its children in one relationship query: `SELECT Name, Industry, AnnualRevenue, Owner.Name, Phone, Website, (SELECT Name, Title, Email, Phone FROM Contacts), (SELECT Name, StageName, Amount, CloseDate FROM Opportunities WHERE IsClosed = false), (SELECT CaseNumber, Subject, Status, Priority FROM Cases WHERE IsClosed = false) FROM Account WHERE Id = :id` |
| Recent activity on it | pull Tasks and Events tied to the account (`WhatId = :id`) ordered by date, and note `LastActivityDate` — surface the last touch and who made it |
| Accounts gone quiet | `SELECT Name, Owner.Name, LastActivityDate FROM Account WHERE LastActivityDate < LAST_N_DAYS:60 ORDER BY LastActivityDate` — flag how long since the last touch |
| Top accounts by revenue | `SELECT Name, AnnualRevenue, Owner.Name FROM Account WHERE AnnualRevenue != null ORDER BY AnnualRevenue DESC LIMIT 25` |
| Accounts by industry | `SELECT Industry, COUNT(Id) FROM Account GROUP BY Industry ORDER BY COUNT(Id) DESC` — count (and total revenue where useful) per industry |
| Accounts by owner | `SELECT Owner.Name, COUNT(Id) FROM Account GROUP BY Owner.Name` — how the book splits across the team |

For a briefing, lead with the headline (who they are, size, owner, health), then the people to know, open deals,
open cases, and the last few touches. Keep it tight and client-ready. Always state the scope of any rollup —
whose accounts, which period — so the numbers are unambiguous. If aggregate queries aren't permitted for the
user, fetch the rows (with a sensible `LIMIT`) and total them myself, and say if the result was capped.

## Field cheat-sheet (Account)

`Name`, `Industry`, `AnnualRevenue`, `NumberOfEmployees`, `BillingCity`, `BillingState`, `BillingCountry`,
`Owner.Name`, `Type`, `Website`, `Phone`, `LastActivityDate`, `CreatedDate`. Related: `Contacts`,
`Opportunities`, `Cases` (children via `AccountId`); `Tasks`/`Events` (via `WhatId`).

Industries, record types, and any custom fields vary per org — read the real values. If the org uses custom
health or segment fields, prefer those and mention it.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset; refer to them generically. This skill is
**read-only**. Return the briefing and any rollups as clear structured text — how it's charted or laid out is the
workspace's job, so don't emit HTML or build visualisations yourself.
