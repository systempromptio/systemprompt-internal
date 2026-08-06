# Manage Calendar

Read and book meetings in the Odoo calendar as the acting user. Events can be linked to the record they
concern, and attendees are real partners — so the calendar is part of the same knowledge graph as the
pipeline, not a silo.

## When to Use

- "What's on today / this week?"
- "Book a demo with Acme Thursday at 2pm."
- After a meeting happens: hand off to `capture_knowledge` (transcript → bank; summary → record note).

## Tools

| Tool | Use for |
|------|---------|
| `calendar_event_list` | Agenda queries: date range, or search by name |
| `calendar_event_create` | Book: name, start (+ stop or duration), attendees, linked record |
| `partner_search` | Resolve attendees to partner ids |
| `crm_lead_search` | Resolve the lead a meeting is about, to link it |

## How to Work

1. **Reading**: default to today/this week; present time, title, location, attendees, linked record.
2. **Booking**: confirm date, time, and duration explicitly — never book from a vague time. Resolve attendees
   via `partner_search`; link the lead when the meeting is about a deal (that puts the meeting on the lead's
   timeline).
3. **Time zones**: state times back in plain terms ("Thursday 14:00"); if the user's timezone is unclear and
   it matters, ask.
4. **After the meeting**: suggest `capture_knowledge` for the transcript and `schedule_activity` for whatever
   was promised — a meeting that produces no captured knowledge and no follow-up is a smell.

## Rules

- Never move or double-book over an existing event without saying so.
- Meetings you create are visible to attendees immediately — book only what was actually agreed.
