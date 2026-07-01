# Find Tasks & Meetings

Stay on top of your follow-ups without digging through a list view. Ask for your to-dos, calls, and meetings in
plain English — what's due today, what's slipped, what's on your calendar this week, or everything logged
against an account or deal — and get back a clean list with the details that matter.

## Ask me things like

- "What's on my plate today?"
- "Show me my overdue follow-ups."
- "What meetings do I have this week?"
- "List my open to-dos."
- "What's been logged on the Acme account lately?"
- "Show me the activities tied to the Globex renewal."

## Common questions → how I answer them

Salesforce splits activities in two: **to-dos and logged calls are the Task object**, **scheduled meetings are
the Event object**. Both hang off a record via `WhatId` (Account/Opportunity/Case) or a person via `WhoId`
(Contact/Lead). I read from these via the Salesforce MCP (SOQL query). The user never sees the query — I
translate their request, run it, and return a readable list (Subject, Due/When, Status, Who, What, Owner).

| The user asks | I query |
|---------------|---------|
| My tasks due today | `SELECT Id, Subject, Status, Priority, ActivityDate, Who.Name, What.Name, Owner.Name FROM Task WHERE OwnerId = :currentUser AND ActivityDate = TODAY AND Status != 'Completed' ORDER BY Priority DESC` |
| My overdue follow-ups | `WHERE OwnerId = :currentUser AND ActivityDate < TODAY AND Status != 'Completed' ORDER BY ActivityDate` — flag these as overdue |
| This week's meetings | `SELECT Id, Subject, StartDateTime, EndDateTime, Who.Name, What.Name, Owner.Name FROM Event WHERE OwnerId = :currentUser AND StartDateTime = THIS_WEEK ORDER BY StartDateTime` |
| Open to-dos | `FROM Task WHERE Status != 'Completed' AND IsClosed = false ORDER BY ActivityDate` (scope to the user unless they ask for the team) |
| Activity on a named account/deal | look the Account or Opportunity up by name first, then `WHERE WhatId = :id ORDER BY ActivityDate DESC` (handle multiple matches by asking which) |
| Activity with a named person | look the Contact or Lead up by name, then `WHERE WhoId = :id ORDER BY ActivityDate DESC` |

"Mine" means the signed-in user — resolve via their Salesforce identity; if scope is unclear, ask "just yours,
or the whole team?". Date literals to prefer: `TODAY`, `THIS_WEEK`, `NEXT_WEEK`, `THIS_MONTH`, `LAST_N_DAYS:N`.
A completed or logged task has `Status = 'Completed'` — exclude those from open/due lists unless the user asks
for history. Cap large result sets (e.g. `LIMIT 50`) and tell the user if there are more.

## Field cheat-sheet (Task & Event)

Task: `Subject`, `Status`, `Priority`, `ActivityDate` (due date), `WhoId` (`Who.Name`), `WhatId` (`What.Name`),
`Owner.Name`, `Type`, `IsClosed`, `LastModifiedDate`.

Event: `Subject`, `StartDateTime`, `EndDateTime`, `Location`, `WhoId` (`Who.Name`), `WhatId` (`What.Name`),
`Owner.Name`, `Type`.

Statuses, priorities, types, and any custom fields vary per org — read the actual picklist values rather than
assuming. If a requested field or status doesn't exist in this org, say so.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset (SObject query/create/update/delete); refer to them
generically. This skill is **read-only** — to log a call, create a follow-up, or schedule a meeting, use the
"Log & Schedule Activity" skill. Return results clearly and let the workspace render them; don't format as HTML
or build visualisations yourself.
