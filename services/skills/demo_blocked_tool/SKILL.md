# Demo — The Blocked Tool

The `tool_blocklist` stage refuses any tool whose **name** contains `delete`, `drop` or `destroy` for
a user-scope caller. The point of this beat is that the tool is real: `crm_lead_delete` genuinely
unlinks a lead in Odoo — record, chatter, activities, gone. Governance is what stands between a
salesperson and that outcome, and it decides by identity, not by the tool being harmless.

## Precondition

Run steps 1–3 as a **non-admin** (`tool_blocklist` exempts admins, as `require_approval` does). Have an
admin ready for step 4. Odoo linked for both.

## The beat

1. **Make something safe to lose.** As the user, `crm_lead_create` with the name
   `DEMO — delete me` and nothing else. Read back its id. This is the only lead this beat may ever
   point `crm_lead_delete` at.

2. **Try to delete it.** `crm_lead_delete` with that id. **Refused** — the reason names the blocklist
   and the substring that matched. The tool never ran: prove it with `crm_lead_get` on the same id.
   The lead is still there.

3. **Say what just happened.** The refusal was decided before the Odoo server was ever spoken to; the
   user's Odoo permissions were not even consulted. Odoo's own rules are a second gate behind this one,
   for the day the first is misconfigured — and the audit row for this call reads
   `policy=tool_blocklist`, not `policy=odoo`.

4. **The same call as an admin.** The admin runs `crm_lead_delete` on the same id. It **executes**:
   the tool reads the lead first (so it can name what it destroyed), unlinks it, and reports
   "irreversible". `crm_lead_get` now says the lead is not visible. Same tool, same arguments, same
   server — the only difference was the caller's live role in the database.

5. **The scope stage, honestly.** `scope_check` refuses a user who calls an admin-only tool
   (`mcp__systemprompt__*`). A user cannot demonstrate that from Cowork, because their manifest does
   not carry the `systemprompt` server at all — the tool is not there to call. Say that plainly; it is
   the manifest doing its job one layer earlier. To show the stage itself firing, an admin posts a
   synthetic `PreToolUse` with a user-scope token, using the recipe in `demonstrate_governance`
   §"Forcing a specific decision".

6. **Read it back** (admin, `governance_readback`): the user's `deny` with `policy=tool_blocklist`, the
   admin's `allow` for the identical tool, seconds apart. Then, if there is time, the live flip in
   `manage_platform` — revoke the admin's role and the same call is refused; re-grant it and it runs —
   with no restart and no new token. Authority is data.

## Rules

- `crm_lead_delete` is pointed only at the lead this beat created. Never at a real one, never on
  inference, never "to tidy up".
- Every verdict is read back from the audit row before it is claimed.
- Hand off: `demo_approval_hold` if you have not run it; otherwise `demonstrate_governance` for the
  full stage-by-stage tour and `lead_factsheet` for the business arc that ends in a held send.
