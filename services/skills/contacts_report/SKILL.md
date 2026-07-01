# Contact Insights

Know who you know at a glance. Ask about the people behind an account and get a clear briefing — who the key
decision-makers are, how your contacts map across accounts, and who's gone quiet — without building a single
report.

## Ask me things like

- "Who are the key people at Acme?"
- "Who are the decision-makers on the Globex account?"
- "Map out all our contacts at Initech by department."
- "Which contacts haven't we spoken to in a while?"
- "How many contacts do we have per account?"
- "Give me a briefing on the people at Stark Industries."

## Common questions → how I answer them

I read and summarise the **Contact** object via the Salesforce MCP. When an account is named I look up the
**Account** by name first, then query Contacts by `AccountId`. Where the org supports aggregate SOQL I group and
count; otherwise I pull the rows and total them myself. I present the result as a plain summary and let the
workspace render it.

| The user asks | How I build it |
|---------------|----------------|
| Key people / decision-makers at an account | look up the Account, then Contacts `WHERE AccountId = :id AND (Title LIKE '%VP%' OR Title LIKE '%Director%' OR Title LIKE '%Chief%' OR Title LIKE '%Head%' OR Title LIKE '%Manager%')` — list by seniority with title and email |
| The contact map for an account | Contacts `WHERE AccountId = :id ORDER BY Department, Title` — grouped by department |
| Contacts by department | `SELECT Department, COUNT(Id) FROM Contact WHERE AccountId = :id GROUP BY Department` |
| Contact counts by account | `SELECT Account.Name, COUNT(Id) FROM Contact WHERE AccountId != null GROUP BY Account.Name ORDER BY COUNT(Id) DESC` |
| Gone quiet / no recent activity | `WHERE LastActivityDate < LAST_N_DAYS:90 OR LastActivityDate = null ORDER BY LastActivityDate` — flag them explicitly |
| A briefing on a named account's people | combine the above: total contacts, the key titles, and anyone with no recent activity |

Always state the scope you used — which account, which period — so the briefing is unambiguous. If aggregate
queries aren't permitted for the user, fall back to fetching rows (with a sensible `LIMIT`) and summing, and say
if the result was capped.

## Field cheat-sheet (Contact)

`Name`, `Account.Name`, `Title`, `Department`, `Email`, `Phone`, `MobilePhone`, `MailingCity`, `Owner.Name`,
`LeadSource`, `LastActivityDate`.

Titles and departments are entered per org and aren't always tidy — match on keywords (VP, Director, Chief,
Head, Manager) rather than exact strings, and mention when a title is ambiguous.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset; refer to them generically. This skill is
**read-only**. Return the summary as clear structured text/numbers — how it's charted or laid out is the
workspace's job, so don't emit HTML or build visualisations yourself.
