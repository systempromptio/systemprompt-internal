# Activity Insights

Your follow-ups and calendar at a glance. Ask how your day or week is shaping up and get a clear rollup — what's
due, what's overdue, how your activity splits by type or owner, and how much attention a given account or deal
has had — without building a single report.

## Ask me things like

- "What's on my plate today?"
- "How busy is my week?"
- "How many follow-ups am I overdue on?"
- "Break down my activity by type."
- "How many times have we touched the Acme account this month?"
- "What meetings are coming up?"

## Common questions → how I answer them

Salesforce splits activities in two: **to-dos and logged calls are the Task object**, **scheduled meetings are
the Event object**. Both relate to a record via `WhatId` (Account/Opportunity/Case) or a person via `WhoId`
(Contact/Lead). I aggregate these via the Salesforce MCP. Where the org supports aggregate SOQL I group and
count; otherwise I pull the rows and total them myself. I present the numbers as a plain summary and let the
workspace render it.

| The user asks | How I build it |
|---------------|----------------|
| Today's plate | open Tasks with `ActivityDate = TODAY` plus Events with `StartDateTime = TODAY`, counted and listed |
| This week's load | Tasks due `THIS_WEEK` (not Completed) and Events `THIS_WEEK`, totalled by day |
| Overdue follow-ups | count of Tasks `WHERE ActivityDate < TODAY AND Status != 'Completed'` — always flag this number explicitly, it's the one that hurts |
| Activity by type | `SELECT Type, COUNT(Id) FROM Task WHERE ... GROUP BY Type` (Call / Email / etc.); do the same over Status to show open vs completed |
| Activity by owner | `GROUP BY OwnerId` (show `Owner.Name`) to compare across the team |
| Touches on an account/deal | look the record up by name, then count Tasks + Events with `WhatId = :id` over a window (e.g. `LAST_N_DAYS:30`) — report it as "N touches" |
| Upcoming meetings | Events with `StartDateTime >= TODAY` ordered ahead, counted and listed |

Always state the scope you used — whose activity, which period — so the numbers are unambiguous. Call out
overdue follow-ups every time they appear, even if the user didn't ask. If aggregate queries aren't permitted
for the user, fall back to fetching rows (with a sensible `LIMIT`) and counting, and say if the result was
capped.

## Field cheat-sheet (Task & Event)

Task: `Subject`, `Status`, `Priority`, `Type`, `ActivityDate` (due date), `WhoId`, `WhatId`, `Owner.Name`,
`IsClosed`.

Event: `Subject`, `StartDateTime`, `EndDateTime`, `Type`, `WhoId`, `WhatId`, `Owner.Name`.

Types, statuses, and any custom fields are configured per org — read the real values. If the org uses custom
activity fields, prefer those and mention it.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset; refer to them generically. This skill is
**read-only**. Return the summary as clear structured text/numbers — how it's charted or laid out is the
workspace's job, so don't emit HTML or build visualisations yourself.
