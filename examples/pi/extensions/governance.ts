/**
 * Enterprise Demo governance for the Pi coding agent.
 *
 * Pi has no hook literally named `PreToolUse`, but it has the equivalents:
 *
 *   Pi event      Claude Code analogue   What this extension does with it
 *   ------------  ---------------------  ----------------------------------------
 *   input         UserPromptSubmit       govern the raw prompt; a denied prompt
 *                                        never reaches a provider at all
 *   tool_call     PreToolUse             govern the tool call; a denied call
 *                                        never executes
 *   tool_result   PostToolUse            record the fire for usage/audit
 *
 * Every decision is made by the gateway, not here: this file only carries the
 * event to `POST /api/public/hooks/govern` and enforces the answer. The four
 * policies (scope_check, secret_scan, tool_blocklist, rate_limit) and the
 * `governance_decisions` audit row are identical to the ones Claude Code hits.
 *
 * Credentials come from ~/.config/systemprompt-pi/, written by
 * examples/pi/setup.sh and examples/pi/new-user.sh:
 *
 *   token       sp-live-… PAT — used by models.json for /v1/messages
 *   hook-token  plugin JWT     — used HERE; /hooks/govern validates a JWT
 *                                (aud = hook|plugin|api) and rejects a PAT
 *   session-id  audit label shared by both
 *   base-url    gateway origin for this deployment's profile
 */

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const CRED_DIR = join(homedir(), ".config", "systemprompt-pi");
const PLUGIN_ID = "enterprise-demo";
const AGENT_ID = "pi_agent";

/**
 * A tool call whose governance verdict is unknown is NOT allowed through.
 *
 * The gateway takes the same posture in reverse: it answers 200 with a deny
 * rather than an error status, because a non-200 reads as "hook unavailable"
 * and a permissive client would run the tool anyway. Flipping this to `true`
 * turns the gate into advice.
 */
const FAIL_OPEN = false;

function readCred(name: string): string {
  try {
    return readFileSync(join(CRED_DIR, name), "utf8").trim();
  } catch {
    return "";
  }
}

const HOOK_TOKEN = readCred("hook-token");
const BASE_URL = (
  process.env.SYSTEMPROMPT_BASE_URL ||
  readCred("base-url") ||
  "http://127.0.0.1:8080"
).replace(/\/$/, "");

/**
 * Audit label for this run. The demo script pins it so its assertions can
 * select exactly the rows one run produced; interactively it falls back to the
 * label new-user.sh wrote, then to Pi's own session id.
 */
function sessionId(ctx: { sessionManager?: { getSessionId?: () => string } }): string {
  return (
    process.env.SYSTEMPROMPT_PI_SESSION ||
    readCred("session-id") ||
    ctx.sessionManager?.getSessionId?.() ||
    "pi-ungoverned"
  );
}

type Verdict = { allowed: boolean; reason?: string };

/**
 * POST one hook event and read the decision. The wire shape is Claude Code's
 * hook contract verbatim — that is the point: Pi is an untouched third-party
 * client speaking the same protocol.
 */
async function govern(
  body: Record<string, unknown>,
  signal?: AbortSignal,
): Promise<Verdict> {
  if (!HOOK_TOKEN) {
    return {
      allowed: FAIL_OPEN,
      reason:
        "no governance credential — run examples/pi/new-user.sh to mint ~/.config/systemprompt-pi/hook-token",
    };
  }
  try {
    const res = await fetch(
      `${BASE_URL}/api/public/hooks/govern?plugin_id=${PLUGIN_ID}`,
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${HOOK_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
        signal,
      },
    );
    if (!res.ok) {
      return { allowed: FAIL_OPEN, reason: `governance unavailable (HTTP ${res.status})` };
    }
    const json = (await res.json()) as {
      hookSpecificOutput?: {
        permissionDecision?: string;
        permissionDecisionReason?: string;
      };
    };
    const out = json.hookSpecificOutput;
    if (out?.permissionDecision === "deny") {
      return { allowed: false, reason: out.permissionDecisionReason ?? "[GOVERNANCE] denied" };
    }
    return { allowed: true };
  } catch (err) {
    if (signal?.aborted) return { allowed: false, reason: "cancelled" };
    return { allowed: FAIL_OPEN, reason: `governance unreachable: ${String(err)}` };
  }
}

/** Usage tracking. Never blocks and never throws into the turn. */
function track(body: Record<string, unknown>): void {
  if (!HOOK_TOKEN) return;
  void fetch(`${BASE_URL}/api/public/hooks/track?plugin_id=${PLUGIN_ID}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${HOOK_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  }).catch(() => {
    /* tracking is best-effort; a dropped usage row must not fail a tool call */
  });
}

export default function (pi: ExtensionAPI) {
  // ── Prompt gate ────────────────────────────────────────────────────────
  // Fires before skill/template expansion and before any provider request, so
  // a credential pasted into the prompt is caught while it is still local.
  pi.on("input", async (event, ctx) => {
    const verdict = await govern(
      {
        hook_event_name: "UserPromptSubmit",
        session_id: sessionId(ctx),
        cwd: process.cwd(),
        agent_id: AGENT_ID,
        prompt: event.text,
      },
      ctx.signal,
    );
    if (!verdict.allowed) {
      ctx.ui.notify(`${verdict.reason} — prompt not sent to the model`, "error");
      return { action: "handled" };
    }
    return { action: "continue" };
  });

  // ── Tool gate (Pi's PreToolUse) ────────────────────────────────────────
  pi.on("tool_call", async (event, ctx) => {
    const verdict = await govern(
      {
        hook_event_name: "PreToolUse",
        session_id: sessionId(ctx),
        cwd: process.cwd(),
        agent_id: AGENT_ID,
        tool_name: event.toolName,
        tool_input: event.input,
        tool_use_id: event.toolCallId,
      },
      ctx.signal,
    );
    if (!verdict.allowed) {
      ctx.ui.notify(String(verdict.reason), "error");
      // The reason goes back to the model, which must explain the denial
      // rather than silently retrying.
      return { block: true, reason: verdict.reason };
    }
    return undefined;
  });

  // ── Post-execution tracking (Pi's PostToolUse) ─────────────────────────
  pi.on("tool_result", (event, ctx) => {
    track({
      hook_event_name: "PostToolUse",
      session_id: sessionId(ctx),
      cwd: process.cwd(),
      transcript_path: "",
      permission_mode: "default",
      tool_name: event.toolName,
      tool_input: event.input,
      tool_response: { isError: Boolean(event.isError) },
      tool_use_id: event.toolCallId,
    });
    return undefined;
  });

  // ── Demo tools ─────────────────────────────────────────────────────────
  // Stock Pi tools are bash/read/write/edit — none of which match
  // scope_check's `mcp__systemprompt__` prefix or tool_blocklist's
  // delete|drop|destroy patterns, so neither policy would ever fire against
  // them. These two exist to be blocked: they stand in for the enterprise MCP
  // surface a real deployment exposes, and their bodies are stubs because a
  // governed call never reaches them.
  pi.registerTool({
    name: "mcp__systemprompt__list_agents",
    label: "List agents (admin)",
    description:
      "List the agents configured on the systemprompt gateway. Admin scope only.",
    parameters: Type.Object({}),
    async execute() {
      return {
        content: [
          {
            type: "text",
            text: "stub — a user-scope caller is denied by scope_check before this runs",
          },
        ],
        details: {},
      };
    },
  });

  pi.registerTool({
    name: "delete_records",
    label: "Delete records",
    description: "Delete rows from a table by name.",
    parameters: Type.Object({
      table: Type.String({ description: "Table to delete from" }),
    }),
    async execute() {
      // Deliberately NOT `mcp__systemprompt__delete_*`: scope_check runs first
      // and would short-circuit an admin-prefixed name, attributing the deny to
      // scope_check. A plain destructive name passes scope_check and is denied
      // by tool_blocklist, so the audit row names the policy that actually
      // fired.
      return {
        content: [
          {
            type: "text",
            text: "stub — a user-scope caller is denied by tool_blocklist before this runs",
          },
        ],
        details: {},
      };
    },
  });
}
