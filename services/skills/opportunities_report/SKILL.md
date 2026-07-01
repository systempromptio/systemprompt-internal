# Pipeline & Forecast

The health of your sales pipeline at a glance. Ask how things are tracking and get a clear rollup — total open
pipeline, what's weighted to close, how deals split across stages, win rate, and what's slipping — without
building a single report.

## Ask me things like

- "How's my pipeline looking this quarter?"
- "What's my forecast for this quarter?"
- "Break my open pipeline down by stage."
- "What's my win rate over the last 90 days?"
- "Which big deals are at risk of slipping?"
- "How does the team's pipeline compare?"

## Common questions → how I answer them

I aggregate the **Opportunity** object via the Salesforce MCP. Where the org supports aggregate SOQL I group and
sum; otherwise I pull the rows and total them myself. I present the numbers as a plain summary (totals, counts,
per-stage breakdown) and let the workspace render it.

| The user asks | How I build it |
|---------------|----------------|
| Pipeline by stage | `SELECT StageName, COUNT(Id), SUM(Amount) FROM Opportunity WHERE IsClosed = false GROUP BY StageName` — order by the stage sequence, show count + total value per stage |
| Total open pipeline | `SUM(Amount)` where `IsClosed = false` for the relevant owner/period |
| Weighted forecast | pull open deals with `Amount` and `Probability`; weighted value = Σ(Amount × Probability/100). Or group by `ForecastCategory` (Pipeline / Best Case / Commit) |
| What's closing this quarter | open deals with `CloseDate = THIS_QUARTER`, totalled and listed by stage |
| Win rate | over a window: `Closed Won ÷ (Closed Won + Closed Lost)` by count (and note by value too) |
| At-risk / slipping | open deals with `CloseDate < TODAY` (past due) or high `Amount` with no recent `LastModifiedDate` — flag them explicitly |
| Team vs mine | group by `Owner.Name`, or filter to the current user; if scope is unclear, ask |

Always state the scope you used — whose deals, which period — so the numbers are unambiguous. If aggregate
queries aren't permitted for the user, fall back to fetching rows (with a sensible `LIMIT`) and summing, and say
if the result was capped.

## Field cheat-sheet (Opportunity)

`StageName`, `Amount`, `Probability`, `ForecastCategory` (Pipeline/Best Case/Commit/Omitted/Closed), `CloseDate`,
`IsClosed`, `IsWon`, `Owner.Name`, `Account.Name`, `LastModifiedDate`.

Stages, probabilities, and forecast categories are configured per org — read the real values. If the org uses
custom forecast fields, prefer those and mention it.

## Notes

Tools come from the Salesforce Hosted `sobject-all` toolset; refer to them generically. This skill is
**read-only**. Return the summary as clear structured text/numbers — how it's charted or laid out is the
workspace's job, so don't emit HTML or build visualisations yourself.
