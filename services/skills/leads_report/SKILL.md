# Lead Insights

The health of your lead funnel at a glance. Ask how prospects are tracking and get a clear rollup — volume by
source, how leads split across statuses, quality by rating, how many are converting, and which open leads have
gone quiet — without building a single report.

## Ask me things like

- "Which of my open leads need follow-up?"
- "Break down my leads by source."
- "How many leads are in each status?"
- "How many leads converted last month versus didn't?"
- "What's the quality mix of my open leads?"
- "Show me leads that have gone stale."

## Common questions → how I answer them

I aggregate the **Lead** object via the Salesforce MCP. Where the org supports aggregate SOQL I group and
count; otherwise I pull the rows and total them myself. I present the numbers as a plain summary (totals,
counts, per-group breakdown) and let the workspace render it.

| The user asks | How I build it |
|---------------|----------------|
| Leads by source | `SELECT LeadSource, COUNT(Id) FROM Lead WHERE IsConverted = false GROUP BY LeadSource` — count per source, largest first |
| Leads by status | `SELECT Status, COUNT(Id) FROM Lead WHERE IsConverted = false GROUP BY Status` — order by the status sequence |
| Leads by rating / quality | `SELECT Rating, COUNT(Id) FROM Lead WHERE IsConverted = false GROUP BY Rating` (Hot / Warm / Cold) |
| Conversion counts | over a window: `SELECT IsConverted, COUNT(Id) FROM Lead WHERE CreatedDate = LAST_MONTH GROUP BY IsConverted` — converted vs not |
| Needs follow-up / stuck | open leads with no recent touch: `WHERE IsConverted = false AND (LastActivityDate < LAST_N_DAYS:14 OR LastActivityDate = null) ORDER BY LastActivityDate` — flag them explicitly |
| Stale open leads | open leads sitting in one status a long time: `WHERE IsConverted = false AND LastModifiedDate < LAST_N_DAYS:30` — call these out so they don't rot |
| My funnel vs the team's | group by `Owner.Name`, or filter to the current user; if scope is unclear, ask |

Always state the scope you used — whose leads, which period — so the numbers are unambiguous. When you spot
open leads with no recent activity or ones stuck in a status, flag them as needing attention. If aggregate
queries aren't permitted for the user, fall back to fetching rows (with a sensible `LIMIT`) and counting, and
say if the result was capped.

## Field cheat-sheet (Lead)

`Status`, `LeadSource`, `Rating`, `Industry`, `IsConverted`, `Owner.Name`, `Company`, `CreatedDate`,
`LastActivityDate`, `LastModifiedDate`.

Statuses, sources, and ratings are configured per org — read the real values. If the org uses custom lead
fields for scoring or quality, prefer those and mention it.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset; refer to them generically. This skill is
**read-only**. Return the summary as clear structured text/numbers — how it's charted or laid out is the
workspace's job, so don't emit HTML or build visualisations yourself.
