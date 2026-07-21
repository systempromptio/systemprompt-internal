import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-setup-gateway.js";
import "/assets/js/components/sp-setup-agents.js";

const STEPS = [
  { id: "connect", label: "Sign in" },
  { id: "agents", label: "Agents" },
];

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
    this.anyInstalled = false;
    this._finished = false;
    this._logoFragment = null;
    this._onSetupOpen = () => { document.body.classList.add("is-setup-mode"); };
    this.registerAction("finish", () => this._finish());
    this.registerAction("open-bridge", () => { document.body.classList.remove("is-setup-mode"); });
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
    const hosts = snap.host_apps || [];
    // Install state for a host is only KNOWN once its probe has completed, at
    // which point `snapshot` is populated. Until every host has a snapshot the
    // result is "unknown" — we must not show onboarding then, or it flashes
    // before detection resolves (the bug where it appeared with agents already
    // installed). Once settled, show the agents step only when none are
    // installed; installing one (anyInstalled) drops straight into the app.
    const settled = hosts.length > 0 && hosts.every((h) => h.snapshot);
    const anyInstalled = hosts.some((h) => h.snapshot?.profile_state?.kind === "installed");
    this.anyInstalled = anyInstalled;

    const needAgents = configured && settled && !anyInstalled && !this._finished;
    const inSetup = !configured || needAgents;
    document.body.classList.toggle("is-setup-mode", inSetup);
    this.step = configured ? "agents" : "connect";
  }

  _finish() {
    this._finished = true;
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

  _renderSteps() {
    const active = STEPS.findIndex((s) => s.id === this.step);
    const items = STEPS.map((s, i) => {
      const state = i < active ? "is-done" : i === active ? "is-current" : "";
      const current = i === active ? 'aria-current="step"' : "";
      return `<li class="sp-setup__step-dot ${state}" ${current}><span>${escapeHtml(s.label)}</span></li>`;
    }).join("");
    return `<ol class="sp-setup__steps" aria-label="Setup progress">${items}</ol>`;
  }

  render() {
    const step = this.step;
    const version = this.dataset.version || "";
    const platform = this.dataset.platform || "linux";
    const platformDisplay = this.dataset.platformDisplay || "";
    // Finish is always enabled. Host install-state is probe-driven and can lag
    // or misreport (e.g. the card shows "Installed ✓" while `anyInstalled` is
    // still false), which trapped the user on this step with no way forward.
    // Installing agents is optional, so never block completing setup.
    const finishDisabled = "";
    return `
      <div class="sp-setup__split">
        <aside class="sp-setup__brand">
          <div class="sp-setup__mark" data-logo-slot></div>
          <div class="sp-setup__pitch">
            <p class="sp-setup__pitch-head">Govern every coding agent.</p>
            <p class="sp-setup__pitch-body">One gateway. Every agent. Every tool call audited.</p>
          </div>
          <footer class="sp-setup__brand-foot">
            <p class="sp-setup__demo">
              <strong data-l10n-id="setup-warning-strong">Demo software.</strong>
              <span data-l10n-id="setup-warning-body">This build is provided for demonstration purposes only and is not licensed for production use.</span>
            </p>
            <p class="sp-setup__meta">
              <span class="sp-setup__version">v${escapeHtml(version)}</span>
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
            ${this._renderSteps()}
            <div class="sp-setup__step" data-step="connect" ${step !== "connect" ? "hidden" : ""}>
              <h1 id="setup-heading">Sign in</h1>
              <p class="sp-setup__lede">
                Use your Astound Salesforce account. Your bridge account is created
                automatically the first time you sign in.
              </p>
              <sp-setup-gateway></sp-setup-gateway>
            </div>
            <div class="sp-setup__step" data-step="agents" ${step !== "agents" ? "hidden" : ""}>
              <h1>Choose your agents</h1>
              <p class="sp-setup__lede" data-l10n-id="setup-agents-lede">Pick the coding agents you want systemprompt bridge to govern.</p>
              <sp-setup-agents></sp-setup-agents>
              <div class="sp-setup__actions">
                <button class="sp-btn-primary" type="button" data-l10n-id="setup-finish" data-action="finish" ${finishDisabled}>Finish</button>
              </div>
            </div>
          </div>
        </section>
      </div>
    `;
  }
}

reactive(SpSetup.prototype, ["snapshot", "step", "anyInstalled"]);
customElements.define("sp-setup", SpSetup);
