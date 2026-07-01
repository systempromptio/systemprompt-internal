# Update Contacts

Keep your people current by just saying what changed. Add a new contact, update a title, email, phone or
department, or reassign the owner — all in plain English. Every change is shown to you and confirmed before it's
saved.

## Ask me things like

- "Add Jane Doe as a new contact at Acme, she's the VP of Sales."
- "Update John Smith's title to Chief Marketing Officer."
- "Change the email on the Globex contact to jane@globex.com."
- "Update the phone number for our Initech contact."
- "Move Jane Doe into the Finance department."
- "Reassign the Stark contacts to me."

## Before any write — always

1. **Find the exact record.** Look the person up by name/account. If more than one matches, list them and ask
   which. Never guess.
2. **Restate the change.** Show the record and every field changing as **old value → new value**. For a new
   contact, show all the values you're about to set.
3. **Get a clear "yes".** Wait for the user's explicit approval. Do not write on a "maybe" or an implied intent.
4. **Then write, and confirm.** Make the change via the MCP and report back what was saved (and the record link
   /Id if available). If Salesforce rejects it (validation rule, required field, permissions), relay the real
   error plainly and stop.

This runs under the user's own Salesforce login — they can only change what their permissions allow.

## Common changes → how I do them

I use the **Contact** object via the Salesforce MCP (update/create SObject).

| The user wants | How I do it |
|----------------|-------------|
| Add a new contact | create Contact with the required fields — at minimum `LastName`, usually `AccountId`; look up the Account first; capture `FirstName`, `Title`, `Email`, `Phone` when given; ask for anything required but missing |
| Update a title | update `Title` |
| Change email / phone | update `Email`, `Phone`, or `MobilePhone` (validate the format looks right before saving) |
| Change department | update `Department` |
| Reassign the owner | update `OwnerId` — resolve the target user by name first and confirm |

Only touch the fields the user mentioned. `LastName` is required on a Contact — never create one without it. Use
real values for this org — if the user's wording doesn't match a valid picklist, show the options and confirm.

## Field cheat-sheet (Contact)

Writable: `FirstName`, `LastName`, `Title`, `Email`, `Phone`, `MobilePhone`, `Department`, `MailingCity`,
`AccountId`, `OwnerId`, `LeadSource`, plus org-specific custom fields.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (create/update/delete SObject); refer to them
generically. Deletes are rare and destructive — treat a delete like any other write but be extra explicit about
what will be removed. Return confirmation as clear text; rendering is the workspace's job.
