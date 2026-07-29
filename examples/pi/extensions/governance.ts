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
 *   token       sp-live-… PAT or session JWT — used by models.json for
 *                                /v1/messages, and here to resolve the session
 *   hook-token  plugin JWT     — used HERE; /hooks/govern validates a JWT
 *                                (aud = hook|plugin|api) and rejects a PAT
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
 * The one session id this Pi run reports, on both spines: the `session_id` in
 * every hook payload and the `x-session-id` header on every provider request.
 * Resolved once, at session start, by resolveSession().
 */
let SESSION_ID = "";
let SESSION_RESOLUTION: Promise<string> | undefined;
let SESSION_ERROR = "";

/** The `session_id` claim of a JWT, or "" for anything that is not one. */
function jwtSessionClaim(credential: string): string {
  const payload = credential.split(".")[1];
  if (!payload || credential.split(".").length !== 3) return "";
  try {
    const json = Buffer.from(payload, "base64url").toString("utf8");
    const claims = JSON.parse(json) as { session_id?: unknown };
    return typeof claims.session_id === "string" ? claims.session_id : "";
  } catch {
    return "";
  }
}

/**
 * A server-issued session available without a network call: the governance
 * JWT's own claim, else the id new-user.sh recorded when it minted one.
 *
 * Both are real rows, so the gateway's attestation passes. The cost is that
 * runs sharing a credential share a timeline, which is why this is a fallback
 * and not the first choice.
 */
function fallbackSession(): string {
  return jwtSessionClaim(readCred("hook-token")) || readCred("session");
}

/**
 * Resolve the session this run is audited under.
 *
 * The gateway attests `x-session-id` against a session row it issued, so there
 * is nothing to invent here — only two ways to obtain one:
 *
 *   JWT credential  it already carries a session_id claim, and the gateway
 *                   requires the header to equal it.
 *   PAT credential  mint a row at POST /api/public/gateway/sessions.
 *
 * SYSTEMPROMPT_PI_SESSION means "use this already-minted session" — the demo
 * script mints one up front so its Part A curl calls and this Pi run share a
 * timeline. It is not a free-form label: an id the server did not issue is
 * rejected on the first provider call.
 *
 * The mint is the one startup step that needs the network. When it fails the
 * extension used to leave SESSION_ID empty, drop the `x-session-id` header, and
 * turn every later provider call into a 400; it now falls back to an
 * already-issued session so the run keeps working.
 */
async function resolveSession(): Promise<string> {
  const pinned = process.env.SYSTEMPROMPT_PI_SESSION;
  if (pinned) return pinned;

  const credential = readCred("token");
  const claimed = jwtSessionClaim(credential);
  if (claimed) return claimed;

  if (!credential) return fallbackSession();

  let res: Response;
  try {
    res = await fetch(`${BASE_URL}/api/public/gateway/sessions`, {
      method: "POST",
      headers: { "x-api-key": credential, "content-type": "application/json" },
    });
  } catch (err: unknown) {
    // A transport failure here (the TUI has been seen to report a bare
    // "fetch failed" where the same call succeeds from curl and from a
    // headless `pi -p`) must not cost the run its session header.
    const reused = fallbackSession();
    if (reused) {
      SESSION_ERROR =
        `could not reach ${BASE_URL} to mint a session; reusing an already-issued one. ` +
        `Governance still applies, but this run shares a timeline with the last one.`;
      return reused;
    }
    throw new Error(
      `could not reach ${BASE_URL} to mint a gateway session (${
        err instanceof Error ? err.message : String(err)
      }) — is the server running? (just start)`,
    );
  }
  if (!res.ok) {
    throw new Error(
      `could not mint a gateway session (HTTP ${res.status}) — check ${CRED_DIR}/token`,
    );
  }
  const { session_id: minted } = (await res.json()) as { session_id?: string };
  if (!minted) throw new Error("gateway returned no session_id");
  return minted;
}

/**
 * Resolve once; every hook awaits the same promise. Never rejects: a failure
 * here must not take a turn down, and it surfaces loudly anyway — without a
 * session the gateway refuses the next provider call outright.
 */
function ensureSession(): Promise<string> {
  SESSION_RESOLUTION ??= resolveSession()
    .catch((err: unknown) => {
      SESSION_ERROR = err instanceof Error ? err.message : String(err);
      return "";
    })
    .then((id) => {
      SESSION_ID = id;
      return id;
    });
  return SESSION_RESOLUTION;
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
  // ── One session, resolved once ─────────────────────────────────────────
  // One Pi conversation is one audited session: governance decisions and
  // `ai_requests` rows share this id, so /admin/demo/trace can show a blocked
  // prompt and the model call it prevented in the same timeline.
  pi.on("session_start", async (_event, ctx) => {
    const id = await ensureSession();
    if (!id) {
      ctx.ui.notify(
        SESSION_ERROR ||
          `no gateway credential — run examples/pi/new-user.sh to write ${CRED_DIR}/token`,
        "error",
      );
    } else if (SESSION_ERROR) {
      // Degraded but working: say so as a warning rather than an error, so the
      // operator knows why two runs share one timeline.
      ctx.ui.notify(SESSION_ERROR, "warning");
    }
  });

  // ── The header the gateway attests ─────────────────────────────────────
  // Unconditional: both credential kinds resolve to a server-issued session,
  // a JWT from its own claim and a PAT from the mint endpoint, so there is no
  // credential shape left to sniff for.
  pi.on("before_provider_headers", (event) => {
    if (SESSION_ID) event.headers["x-session-id"] = SESSION_ID;
  });

  // ── Prompt gate ────────────────────────────────────────────────────────
  // Fires before skill/template expansion and before any provider request, so
  // a credential pasted into the prompt is caught while it is still local.
  pi.on("input", async (event, ctx) => {
    const verdict = await govern(
      {
        hook_event_name: "UserPromptSubmit",
        session_id: await ensureSession(),
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
        session_id: await ensureSession(),
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
  pi.on("tool_result", (event) => {
    track({
      hook_event_name: "PostToolUse",
      session_id: SESSION_ID,
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
