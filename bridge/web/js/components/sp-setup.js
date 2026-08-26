import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-setup-gateway.js";

function isConfigured(snap) {
  const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
  const id = snap.verified_identity;
  return !!(reachable && id && id.user_id);
}

export class SpSetup extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.step = "connect";
    this._finished = false;
    /** Latched once the app proper is on screen; see `_applySnapshot`. */
    this._leftSetup = false;
    this._logoFragment = null;
    this._onSetupOpen = () => { document.body.classList.add("is-setup-mode"); };
    this.registerAction("finish", () => this._finish());
    this.registerAction("sign-out", () => { bridge.logout().catch((e) => console.warn("logout", e)); });
    this.registerAction("open-bridge", () => { this._leftSetup = true; document.body.classList.remove("is-setup-mode"); });
  }

  onConnect() {
    const tpl = this.querySelector('template[data-slot="logo"]');
    if (tpl) {
      this._logoFragment = tpl.content;
      tpl.remove();
    }
    bridge.stateSnapshot().then((s) => this._applySnapshot(s)).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => this._applySnapshot(s));
    this._unsubOpen = onBridgeEvent("setup-open", this._onSetupOpen);
  }

  onDisconnect() {
    if (this._unsubOpen) { this._unsubOpen(); this._unsubOpen = null; }
  }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (!snap) { return; }
    const configured = isConfigured(snap);
    // Onboarding is sign in, confirm who you are, done. Agents are managed in
    // the Agents tab afterwards (and the first-run pass installs detected
    // hosts automatically) — the old agents step duplicated that surface.
    this.step = configured ? "done" : "connect";

    // Signing out is the one thing that legitimately sends us back to the
    // splash. Clear the latch so it can.
    if (!snap.verified_identity || !snap.verified_identity.user_id) { this._leftSetup = false; }

    // Only decide overlay visibility on a settled gateway probe — partial
    // snapshots flip `configured` mid-startup and made the window flick
    // splash → app → splash.
    const gatewayProbing = !snap.gateway_status || snap.gateway_status.state === "probing"
      || snap.gateway_status.state === "unknown";
    if (gatewayProbing) { return; }

    // One-way latch: once the app proper has been shown, a later probe result
    // must not yank the user back into onboarding mid-session.
    if (this._leftSetup) { return; }

    // `agents_onboarded` is the durable sentinel written by Finish; without
    // consulting it a returning user re-entered onboarding on every launch.
    const onboarded = this._finished || !!snap.agents_onboarded;
    const inSetup = !configured || !onboarded;
    if (!inSetup) { this._leftSetup = true; }
    document.body.classList.toggle("is-setup-mode", inSetup);
  }

  _finish() {
    this._finished = true;
    this._leftSetup = true;
    bridge.setupComplete().catch((err) => console.warn("setup complete", err));
    document.body.classList.remove("is-setup-mode");
  }

  afterRender() {
    document.body.dataset.setupStep = this.step;
    const slot = this.querySelector("[data-logo-slot]");
    if (slot && this._logoFragment && !slot.firstElementChild) {
      slot.append(this._logoFragment.cloneNode(true));
    }
  }

  _identityEmail() {
    const id = this.snapshot && this.snapshot.verified_identity;
    return (id && (id.email || id.user_id)) || "";
  }

  render() {
    const step = this.step;
    const version = this.dataset.version || "";
    const platform = this.dataset.platform || "linux";
    const platformDisplay = this.dataset.platformDisplay || "";
    return `
      <div class="sp-setup__split">
        <aside class="sp-setup__brand">
          <div class="sp-setup__mark" data-logo-slot data-preserve></div>
          <div class="sp-setup__pitch">
            <p class="sp-setup__pitch-head">Govern every coding agent.</p>
            <p class="sp-setup__pitch-body">One gateway. Every agent. Every tool call audited.</p>
          </div>
          <footer class="sp-setup__brand-foot">
            <p class="sp-setup__meta">
              <span class="sp-setup__version">Systemprompt Internal Bridge v${escapeHtml(version)}</span>
              <span class="sp-setup__meta-sep">·</span>
              <a class="sp-setup__docs" href="https://systemprompt.io/docs/bridge/${escapeHtml(platform)}" target="_blank" rel="noopener noreferrer">
                Documentation for ${escapeHtml(platformDisplay)} →
              </a>
              <span class="sp-setup__meta-sep">·</span>
              <a href="mailto:ed@systemprompt.io?subject=systemprompt%20bridge%20licensing">Licensing</a>
            </p>
          </footer>
        </aside>

        <section class="sp-setup__panel">
          <div class="sp-setup__panel-inner">
            <div class="sp-setup__step" data-step="connect" ${step !== "connect" ? "hidden" : ""}>
              <h1 id="setup-heading">Sign in</h1>
              <p class="sp-setup__lede">
                Your browser opens to sign in — use your Odoo email and password
                (or API key), or a passkey. Whoever you approve there is the
                account this computer links to.
              </p>
              <sp-setup-gateway></sp-setup-gateway>
            </div>
            <div class="sp-setup__step" data-step="done" ${step !== "done" ? "hidden" : ""}>
              <h1>You're connected</h1>
              <p class="sp-setup__lede">This computer is linked to</p>
              <p class="sp-setup__identity">${escapeHtml(this._identityEmail())}</p>
              <p class="sp-setup__hint">
                Not you? <a href="#" data-action="sign-out">Sign out</a> and sign
                in again — you choose the account in the browser. Coding agents
                are managed in the Agents tab once you're in.
              </p>
              <div class="sp-setup__actions">
                <button class="sp-btn-primary" type="button" data-l10n-id="setup-finish" data-action="finish">Finish</button>
              </div>
            </div>
          </div>
        </section>
      </div>
    `;
  }
}

reactive(SpSetup.prototype, ["snapshot", "step"]);
customElements.define("sp-setup", SpSetup);
