# Team Chat

Speak into the team's Odoo Discuss channels as the acting user. A channel post reaches the team where they
already talk — and because Discuss messages live in the same `mail.message` table as record notes, everything
posted stays retrievable through `note_search` forever.

## When to Use

- "Post the Acme summary to #sales."
- Announce a milestone the team should see now, not find later ("lead 45 moved to Won").
- Share a daily brief or a knowledge-bank find with a channel.

## Tools

| Tool | Use for |
|------|---------|
| `channel_list` | Find the right channel by name; see what channels exist |
| `channel_post` | Post the message into the channel, authored as you |
| `note_search` | Confirm whether something was already shared before repeating it |

## How to Work

1. **Resolve the channel first** with `channel_list`; if the name is ambiguous, ask. Never guess a channel id.
2. **Write for chat**: short, lead with the point, link records by name and id ("Lead 45 — E2E: Acme rollout").
   A channel post is a headline; the chatter note on the record is the archive copy.
3. **Don't double-post.** One channel per message unless the user names several.
4. **Record-first discipline**: if the update belongs on a record (a decision about a lead), post the note
   there first (`note_add`), then headline it in the channel.

## Rules

- Never post confidential record details to a broad channel without being asked to.
- A failed post (no access to the channel) is Odoo's answer — report it, don't work around it.
