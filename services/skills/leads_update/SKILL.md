# Update Leads

Keep your leads current by just saying what changed. Capture a brand-new lead, move a lead to the next status,
update its rating, reassign the owner, or fix contact details — all in plain English. Every change is shown to
you and confirmed before it's saved.

## Ask me things like

- "Create a new lead: Jane Doe at Acme Corp."
- "Move the Globex lead to Working."
- "Mark the Stark Industries lead as Qualified."
- "Set the Wayne Enterprises lead to Hot."
- "Reassign the Initech lead to Sam."
- "Update the email on the Acme lead to jane@acme.com."

## Before any write — always

1. **Find the exact record.** Look the lead up by name/company. If more than one matches, list them and ask
   which. Never guess.
2. **Restate the change.** Show the record and every field changing as **old value → new value**. For a new
   lead, show all the values you're about to set.
3. **Get a clear "yes".** Wait for the user's explicit approval. Do not write on a "maybe" or an implied intent.
4. **Then write, and confirm.** Make the change via the MCP and report back what was saved (and the record link
   /Id if available). If Salesforce rejects it (validation rule, required field, permissions), relay the real
   error plainly and stop.

This runs under the user's own Salesforce login — they can only change what their permissions allow.

## Common changes → how I do them

I use the **Lead** object via the Salesforce MCP (update/create SObject).

| The user wants | How I do it |
|----------------|-------------|
| Create a lead | create Lead with the required fields — at minimum `LastName` and `Company`; add `FirstName`, `Title`, `Email`, `Phone`, `LeadSource`, `Status` when given; ask for anything required but missing |
| Move a status | update `Status` to a valid picklist value (e.g. Working, Qualified) — match the org's real status picklist |
| Update rating | update `Rating` (e.g. Hot / Warm / Cold) to a valid picklist value |
| Reassign owner | update `OwnerId` — look the target user up by name first; handle multiple matches by asking which |
| Fix contact details | update `Email`, `Phone`, `Title`, or `Company` as named |
| "Convert" the lead | see below — true conversion is a special Salesforce action |

Only touch the fields the user mentioned. Use real picklist values for this org — if the user's wording doesn't
match a valid status or rating, show the options and confirm.

### Converting a lead

Converting a lead into an Account, Contact, and Opportunity is a **special Salesforce action**, not an ordinary
field edit — the basic SObject toolset may not expose it. If a dedicated convert tool is available, use it and
confirm as with any other write. If it isn't, set `Status` to the org's qualified/converted value and tell the
user plainly that the full conversion (creating the Account/Contact/Opportunity) may need to be finished inside
Salesforce. Never pretend a lead was fully converted when only its status was changed.

## Field cheat-sheet (Lead)

Writable: `FirstName`, `LastName`, `Company`, `Title`, `Email`, `Phone`, `Status`, `LeadSource`, `Rating`,
`Industry`, `OwnerId`, plus org-specific custom fields. Required to create: `LastName` and `Company`.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (create/update/delete SObject); refer to them
generically. Deletes are rare and destructive — treat a delete like any other write but be extra explicit about
what will be removed. Return confirmation as clear text; rendering is the workspace's job.
