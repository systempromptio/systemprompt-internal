# Find Contacts

Find the people you need without touching a filter or a report. Ask for contacts by name, account, title,
email, or phone — in plain English — and get back a clean list with the details that matter.

## Ask me things like

- "Find John Smith's contact details."
- "Who do we know at Acme?"
- "Show me the VPs and Directors at Globex."
- "Look up the contact with this email address."
- "Which contacts are mine?"
- "Find everyone in the marketing department at Initech."

## Common questions → how I answer them

I read from the **Contact** object via the Salesforce MCP (SOQL query). The user never sees the query — I
translate their request, run it, and return a readable list (Name, Account, Title, Email, Phone, Owner).

| The user asks | I query |
|---------------|---------|
| A person by name | `SELECT Id, Name, Account.Name, Title, Email, Phone FROM Contact WHERE Name LIKE '%Smith%' ORDER BY LastName` — if several match, list them and ask which |
| Who we know at an account | look up the Account by name first, then `WHERE AccountId = :id` (handle multiple account matches by asking which) |
| By title / role | `WHERE Title LIKE '%VP%'` (or Director/Manager/Head) — combine with an account filter when named |
| By email or phone | `WHERE Email = 'name@company.com'` or `WHERE Phone LIKE '%1234%'` / `MobilePhone LIKE '%1234%'` |
| My contacts (the signed-in user's) | add `AND OwnerId = :currentUser` — resolve via the user's Salesforce identity; if unsure, ask "just yours, or the whole team?" |
| By department | `WHERE Department = 'Marketing'` — combine with an account filter when named |
| By location | `WHERE MailingCity = 'London'` |

Names can be fuzzy — search on `Name`, `FirstName`, and `LastName` and confirm when more than one person fits.
Cap large result sets (e.g. `LIMIT 50`) and tell the user if there are more.

## Field cheat-sheet (Contact)

`Name` (`FirstName` / `LastName`), `Account.Name`, `Title`, `Email`, `Phone`, `MobilePhone`, `Department`,
`MailingCity`, `Owner.Name`, `LeadSource`, `LastActivityDate`.

Departments, titles, and any custom fields vary per org — read the actual values rather than assuming. If a
requested field doesn't exist in this org, say so.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (SObject query/create/update/delete); refer to them
generically. This skill is **read-only** — to change a contact, use the "Update Contacts" skill. Return results
clearly and let the workspace render them; don't format as HTML or build visualisations yourself.
