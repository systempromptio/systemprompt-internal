# Update Cases

Keep your support cases current by just saying what changed. Move a case to Working or Escalated, change its
priority, reassign the owner, add a resolution or comment, or close it out — and create brand-new cases — all in
plain English. Every change is shown to you and confirmed before it's saved.

## Ask me things like

- "Move the Acme printer case to Working."
- "Escalate case 00123."
- "Change the priority on the Initech case to High."
- "Reassign the Globex cases to Dana."
- "Add a resolution to case 00456 — 'replaced the faulty cable'."
- "Close the Wayne case, it's resolved."
- "Open a new case for Stark Industries about a login problem."

## Before any write — always

1. **Find the exact record.** Look the case up by number/account/subject. If more than one matches, list them
   and ask which. Never guess.
2. **Restate the change.** Show the record and every field changing as **old value → new value**. For a new
   case, show all the values you're about to set.
3. **Get a clear "yes".** Wait for the user's explicit approval. Do not write on a "maybe" or an implied intent.
4. **Then write, and confirm.** Make the change via the MCP and report back what was saved (and the record link
   /CaseNumber if available). If Salesforce rejects it (validation rule, required field, permissions), relay the
   real error plainly and stop.

This runs under the user's own Salesforce login — they can only change what their permissions allow.

## Common changes → how I do them

I use the **Case** object via the Salesforce MCP (update/create SObject).

| The user wants | How I do it |
|----------------|-------------|
| Update status | update `Status` to a valid picklist value (e.g. `Working`, `Escalated`); confirm the wording matches the org's real status list |
| Escalate | set `Status` to the org's escalated value and/or `IsEscalated = true`, per how the org models it |
| Change priority | update `Priority` to a valid picklist value (e.g. `High`) |
| Reassign owner | update `OwnerId` — look the new owner up by name first; if several match, ask which |
| Add a resolution / comment | update the resolution field the org uses (e.g. a description/comments field), or add a case comment; ask if it's unclear where the note should go |
| Close a case | set `Status` to a **Closed** value — closing sets `IsClosed` accordingly; offer to capture a resolution if the org expects one |
| Create a case | create Case with the required fields — at minimum `Subject`, usually `Status` and `Origin`, often `AccountId` and `ContactId`; look up the Account/Contact first; ask for anything required but missing |

Only touch the fields the user mentioned. Use real picklist values for this org — if the user's wording doesn't
match a valid status or priority, show the options and confirm.

## Field cheat-sheet (Case)

Writable: `Subject`, `Status`, `Priority`, `Origin`, `IsEscalated`, `AccountId`, `ContactId`, `OwnerId`,
plus the org's resolution/comment fields and any org-specific custom fields.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (create/update/delete SObject); refer to them
generically. Deletes are rare and destructive — treat a delete like any other write but be extra explicit about
what will be removed. Return confirmation as clear text; rendering is the workspace's job.
