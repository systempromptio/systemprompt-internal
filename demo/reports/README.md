# Month-end reports

Two pages, one dataset, opposite audiences.

| Page | Who | What it says |
|------|-----|--------------|
| `/admin/reports/internal` | Platform admins only | Licence revenue against provider cost, per customer, plus margin, budget health, and unit economics |
| `/admin/reports/customer` | Any admin, scoped to one organization | Seats, tokens, and breakdowns by department, model, and user — plus the licence fee, and no internal figure at all |

Both are scoped to a **calendar month**, selected with `?month=YYYY-MM`, and
default to the last complete month. Both carry a Print / Save PDF button; the
PDF a customer receives is the browser's own print of the page, so there is no
second rendering path to drift.

A platform admin can read any organization's customer report with
`?org=<slug>`. Anyone else gets their own organization and has no way to name
another's — the parameter is ignored for them.

## Running the demo

```bash
demo/reports/01-seed-report-data.sh    # two enterprises, ~50 users, 6 months
demo/reports/03-smoke.sh               # seed, assert, restore
demo/reports/02-unseed-report-data.sh  # restore by hand
```

The seed is **insert-only**. Every row it writes carries an `rptseed-` id and
nothing pre-existing is updated, which is what makes the unseed an exact
restore rather than a cleanup. If an organization already exists under one of
the demo slugs and this script did not create it, the seed aborts rather than
mutating it.

## What the seed produces

- **Astound Digital** — enterprise plan, 40 users across Engineering, Product,
  Design, Sales, and Support
- **systemprompt** — team plan, 12 users across Engineering and Growth
- Six complete months of requests across `claude-opus-4`, `claude-sonnet-4`,
  `claude-haiku-4`, and `gpt-4o`, weighted so the cheap model carries the call
  volume and the expensive one carries the spend
- Volume that grows month over month, so the trend chart slopes
- A ~1.8% failure rate, so the success-rate tile is not a flat 100%

Timestamps land strictly inside a month, never on a boundary: the reports use
half-open month bounds, and a row on the edge would be counted in the wrong
month.
