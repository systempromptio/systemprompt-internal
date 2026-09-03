---
title: "Governing Enterprise AI Across Five Layers"
source: whitepaper
project: positioning
author: "Roman Martynenko"
published: "2026-09"
---

# Governing Enterprise AI Across Five Layers

A map of the AI governance market, the case against buying governance from the platforms being
governed, and why the decisive capability is expertise rather than tooling.

Roman Martynenko · September 2026

## Abstract

Enterprise AI spending will approach $2.6 trillion in 2026 while more than half of generative AI
projects are abandoned after proof of concept and roughly 80% of adopters report no material
earnings impact. The consensus explanation — that value is gated by governance, process fit, data
readiness, and adoption rather than by model capability — has produced a market in which every major
vendor now sells a "control plane" for AI. This paper sorts that market into five layers by what
each governs and who buys it, and draws three conclusions. Governance mechanisms are commoditizing
and will not differentiate any vendor for long. The market is consolidating fast: security vendors
are buying the gateways, and platform vendors are building governance into their own estates. That
confronts buyers with a paradox — governance is the layer that keeps every other technology choice
reversible, and buying it from your platform vendor makes the platform the one choice you cannot
reverse. And the persistent failures are not technical, which means the decisive capability for an
enterprise is not which governance tool it buys but the expertise with which the tools are chosen,
bounded, and operated. The paper closes with a worked design for the most common client archetype,
an enterprise anchored on a single platform, and an account of the trade-offs between defaulting to
the vendor and pursuing independent governance.

## 1. Why governance became the bottleneck

The first wave of enterprise generative AI resolved a question nobody now disputes: the models work.
What it left unresolved is why so little of that capability has reached the profit and loss
statement. Gartner reports that more than 50% of generative AI projects were abandoned after proof
of concept by the end of 2025 — worse than its own 2024 prediction of 30% — and forecasts that more
than 40% of agentic AI projects will be cancelled by the end of 2027, citing cost escalation,
unclear value, and inadequate risk controls. McKinsey finds that nearly 80% of companies have
deployed generative AI while roughly the same share report no material earnings impact, and that
fewer than 10% of high-value, function-specific use cases escape pilot. Gartner's productivity
research supplies the mechanism: individual desk workers save about four hours a week with AI tools,
but measured team-level gains collapse to roughly ninety minutes with no correlation to output
quality. Individual adoption does not aggregate into organizational performance unless the
surrounding work is redesigned.

The industry has drawn the right conclusion from this, and then drawn a commercial one from it. The
right conclusion is that enterprise AI value is multiplicative — capability times process fit times
data readiness times reliability times adoption times governance times economics — so that the
weakest factor sets the outcome, and the governance factor is where most enterprises are weakest as
agents acquire the authority to change state rather than merely answer questions. The risk of an AI
error scales with the authority, autonomy, reach, and irreversibility granted to the system, which
means total risk can rise even as models improve. Gartner expects 40% of enterprises to demote or
decommission autonomous agents by 2027 because governance gaps surface only after production
incidents.

The commercial conclusion is that governance is the thing to sell. Salesforce, ServiceNow,
Microsoft, Palo Alto Networks, IBM, the hyperscalers, and a dozen funded startups now describe what
they offer as a control plane for AI. The vocabulary has converged faster than the products, and an
executive trying to buy governance today faces a market in which everything is called the same thing
and governs different things. The first job of this paper is to sort it out.

## 2. The five layers

The five layers are easiest to grasp through the life of a single AI action: an employee directs an
AI agent, the agent's request passes through a gateway, and the work lands on AI models, on company
systems, or on other governed agents — with the security layer watching the traffic throughout and
the oversight layer keeping the record that proves afterwards what happened.

**Figure 1 — the five layers in the life of one AI action**

- **Layer 5 — the workforce layer** sets the rules for the AI your people use at their desks: who
  may do what, and a record of who did what.
- **Layer 1 — the gateway** is the switchboard: every request an agent makes to an AI model or a
  company system passes through one point where it is checked, priced, and logged.
- **Layer 2 — the control plane** is air-traffic control for AI workers operating inside your
  business systems: which agent handles which task, and what each one is cleared to touch.
- **Layer 3 — security** is the cameras and alarms on all of that traffic: it spots intrusions,
  leaks, and misbehaving agents.
- **Layer 4 — the oversight layer** is the compliance record — the file you show the regulator: what
  your rules are, and proof that you followed them.

Figure 2 sorts the market onto this stack by what each layer governs and who signs the purchase
order, because those two facts predict a product's behaviour better than its feature list. No single
vendor covers the stack, though several claim to, and the claim is usually true within the vendor's
own domain and false outside it.

Three dynamics run across every layer and matter more than any single vendor. First, the mechanisms
are commoditizing. Pre-execution policy checks, tool-level access control, secret and PII
guardrails, budgets, and audit trails are now present in open-source gateways, in Salesforce's Flex
Gateway, and in the small self-hosted projects alike; no one will win on that checklist for long,
and any vendor whose pitch rests on it is describing table stakes. Second, consolidation is coming
from two directions at once. Security vendors are buying the gateways — Palo Alto Networks closed
its acquisition of Portkey, the highest-volume open-source gateway, in June 2026 — to own the
runtime chokepoint, while platform vendors are embedding control planes in their estates so that
governance arrives as a feature of the system of record. Both squeeze standalone governance products
that compete on features. Third, and most important, the persistent gap is not technical.
Salesforce's own implementation partners note that "deterministic guardrails do not fix bad data";
McKinsey's agentic report concludes that "the main challenge won't be technical — it will be human";
the analysts who covered the Portkey acquisition observed that a central gateway only governs the
traffic that routes through it, and that in real engineering organizations shadow AI and local agent
experimentation limit how much any central plane actually sees. Every layer of the map sells a tool.
The failures happen in the work around the tool.

**Figure 2 — the AI governance landscape in five layers.** Sorted by what each layer governs and who
buys it. The last column states where independent advisory work adds value in each layer.

| Layer | What it governs · who buys | Representative players | Market dynamic | Where advisory value sits |
|---|---|---|---|---|
| **1 · The gateway layer** — model & tool gateways (infrastructure) | Every LLM and MCP tool call an application or agent makes: routing, failover, budgets, guardrails, audit. Unit: application, API key, team. Buyer: platform engineering, CISO. | Portkey (now Palo Alto Prisma AIRS) · TrueFoundry · Bifrost (Maxim AI) · LiteLLM · Kong AI Gateway · hyperscaler-native gateways (AWS Bedrock, Azure AI Foundry, Google Vertex) | Commoditizing fast. Open-source cores (Portkey MIT, Bifrost, LiteLLM); pre-execution checks, tool-level RBAC and PII/secret guardrails are table stakes. Being absorbed into security platforms and clouds. | Gateway selection for the estate; design of the policy model, cost attribution, and model-routing rules that run on it. |
| **2 · The control-plane layer** — agent orchestration (platform) | Declared networks of agents acting across enterprise systems: registry, routing, agent-to-agent and agent-to-tool policy, observability. Unit: agent, workflow. Buyer: CIO, integration leadership. | Salesforce / MuleSoft Agent Fabric · ServiceNow AI Control Tower · Microsoft Foundry & Copilot Studio · AWS Bedrock AgentCore · Google Vertex Agent Builder | Every system-of-record vendor is building one. "Multi-vendor" means other agents plug into their plane; gravity pulls toward the vendor's own agents. Heavy prerequisites; per-agent cost and audit tracing still partly roadmap. | Process redesign, data readiness, evaluation frameworks, and the boundary decision: where the platform's plane should stop. |
| **3 · The security layer** — AI runtime protection | Threats in AI traffic: prompt injection, data exfiltration, malicious tools, rogue agents. Unit: traffic, threat. Buyer: CISO, SOC. | Palo Alto Prisma AIRS · CrowdStrike · Zscaler · Cloudflare (AI Gateway, Firewall for AI) · Cisco AI Defense · Microsoft Defender & Purview | Consolidators. Security vendors are buying gateways (Palo Alto–Portkey, June 2026) to own the runtime chokepoint. Detection-and-response posture; central planes only see traffic routed through them. | Integration with the incumbent security stack; threat modelling for agent authority and reach. Rarely a place to introduce a new vendor. |
| **4 · The oversight layer** — governance, risk & compliance platforms | Inventory of models and use cases, risk classification, regulatory obligations (EU AI Act), policy attestations. Unit: use case, obligation. Buyer: Chief Risk Officer, General Counsel, compliance. | IBM watsonx.governance · Credo AI · Holistic AI · OneTrust AI Governance · ServiceNow · Microsoft Purview | Regulation-driven. Strong on policy and inventory, weak on runtime enforcement — they document what should happen; layers 1–3 control what does. The "policy PDF" problem. | Governance operating-model design: graduated autonomy, control plane before scale, outcome metrics. The bridge between policy and enforcement. |
| **5 · The workforce layer** — governed adoption of the AI people use | The agents employees actually use — Claude, Codex, Copilot, ChatGPT — acting on company systems: who did what, with which data, under which rules; skills and connectors delivered per role. Unit: a named person's action. Buyer: CEO, COO, CISO. | Vendor-captive admin controls (Microsoft Copilot + Purview + Intune, ChatGPT Enterprise, Claude Enterprise) · shadow-AI detection (Netskope, Nightfall, Harmonic) — detection, not governance · an emerging class of open-source and source-available, self-hosted workforce-governance projects (e.g. systemprompt.io) | Nascent; no neutral incumbent. This is where central gateways go blind (shadow AI, local agents). Largest gravitational pull: Microsoft bundling; open-source gateways adding endpoint agents. | Governed-adoption programs: role-based rollout, training, value measurement, and the cultural conditions under which gains are surfaced rather than hidden. |

## 3. What the map says about buying governance

Read across, the map yields four observations that an executive should carry into any procurement
conversation.

**The gateways (layer 1) are becoming infrastructure in the plumbing sense** — necessary,
interchangeable, and increasingly free. With Portkey's core under an MIT licence, Bifrost and
LiteLLM open source, and every hyperscaler shipping a native gateway, the buying decision is about
operational fit and who runs it, not about capability. The value an enterprise extracts from this
layer comes from the policy model and cost attribution it designs, not from the gateway it picks.

**The control planes (layer 2) are where the dependency question is sharpest.** Each is genuinely
capable within its estate; each describes itself as multi-vendor; and in each case "multi-vendor"
means that other vendors' agents can be registered in this vendor's plane. The gravity always pulls
toward the platform's own agents, and the prerequisites are heavy — Salesforce's Agent Fabric
presumes Anypoint and a pair of Flex Gateways, and its per-agent cost tracing and end-to-end audit
are, by the vendor's own architecture documentation, still partly roadmap. These are the right tool
inside their domain and the wrong tool as an enterprise-wide referee.

**The oversight layer's platforms (layer 4) have the opposite problem:** they are neutral by design
and toothless by construction. They inventory, classify, and attest; they do not enforce. An
enterprise that buys one and stops has a very good description of the governance it does not have.
The layer earns its place only when every enforcing layer emits evidence to it, which is an
integration and operating-model problem, not a product feature.

**The workforce layer (layer 5) is the least mature and the most consequential,** because it is
where the people are. It governs the agents employees actually use — in the productivity suite, in
the IDE, in the browser — acting on company systems, and its unit of account is a named person's
action rather than an application key. No neutral incumbent exists. The vendor-captive controls
govern only that vendor's tools; the shadow-AI products detect rather than govern; and the emerging
class of open-source and source-available projects — self-hosted, single-binary systems that ship
signed, role-scoped bundles of skills, connectors, and policies to each person's machine — addresses
precisely the blind spot the gateway analysts identified, at the cost of scale, certification, and
distribution that the incumbents have. The most probable entrant is the productivity-suite vendor
bundling its admin controls into a de facto workforce layer. That is the dynamic to watch.

## 4. Tools versus expertise

The map poses a question to every party in it, including consulting and integration firms: is the
durable position in this market a tools play or an expertise play? The evidence points hard in one
direction.

A tools play means owning a product in one layer and competing for platform budget against the
consolidators named on the map. It requires roadmap velocity, security certifications, and
distribution at a scale only the consolidators possess, and it is a venture-scale bet in a category
whose mechanisms are commoditizing on a two-to-three-year horizon. For a consulting firm it carries
a second cost: an advisor that carries its own governance product into an account it also implements
forfeits the neutrality that makes its advice worth paying for, and invites channel conflict with
the platforms it depends on. The binaries will be commoditized; the open-source class is proof that
the price of the mechanism is trending toward zero.

An expertise play means being the party that makes any of these tools produce a business outcome,
and the map shows that this work is the same in every layer. It is architecture selection: the
platform's plane for agents in the platform's estate, an open gateway for the engineering
organization, an owned layer for the workforce, and the judgment to say which and where each should
stop. It is the governance operating model that none of the tools supply — autonomy graduated and
matched to authority (retrieve, recommend, act with approval, bounded autonomy, autonomous
workflow), the control plane built before agent count grows rather than after the first incident,
and outcome metrics (cost per successful outcome, cycle-time reduction, error cost) in place of
adoption metrics (licences, tokens, agents). It is the unglamorous prerequisites every vendor's own
partners keep naming: data readiness, process documentation, evaluation frameworks, policy drift
management, and the work redesign without which copilots on old workflows reproduce the pattern
Gartner documented, in which the hours saved by individuals never appear in team output. And it is
adoption itself — role-based rollout, training, and the cultural conditions under which people
surface productivity gains rather than hide them, which in organizations where efficiency has
historically meant headcount reduction is the single most underestimated factor in the
multiplicative chain.

Tool-agnosticism is what makes the expertise play credible, and for an integrator it is not a
posture to adopt but the position the role already occupies. A firm with no product to protect can
recommend the platform's plane to a platform client, the incumbent security vendor's gateway to a
CISO, and an owned workforce layer to a board that wants sovereignty, and be believed in each case.
That neutrality is worth more than any single product position, and it compounds: every
implementation across every layer adds to an institutional body of knowledge about what works, which
is the asset a consulting firm actually sells.

## 5. A worked design: the platform-anchored enterprise

The most common enterprise archetype is anchored on a single platform — Salesforce is used here
because it is the most fully developed example, but the analysis transfers to a ServiceNow, SAP, or
Microsoft-anchored estate with minor changes. The archetype has a commerce or service cloud at the
core, a data cloud arriving, an integration platform either licensed or being proposed as the price
of admission to the vendor's agent control plane, and the vendor's agents being pitched hard by the
account team.

Two facts about this enterprise matter before any tooling decision. It is already deeply dependent
on the platform for its system of record, so the question is never whether to depend on the platform
but where the dependency should stop. And it is not a platform company — its workforce lives in a
productivity suite from a different vendor, its engineers live in source control and their coding
agents, its data estate likely spans a warehouse from a third vendor, and its people already use
Claude, ChatGPT, and Copilot whether or not anyone approved them. The platform's governance tooling
governs the platform's domain; the enterprise's AI exposure is wider than that domain.

### The design

One principle resolves the design: **govern within a vendor's domain with that vendor's tools;
govern across domains with tools the company owns or controls.** Applied to the five layers it
produces the posture below.

| Layer | Posture for a platform-anchored enterprise (Salesforce example) | Rationale |
|---|---|---|
| 1 · Gateway layer | **Independent.** An enterprise LLM and MCP gateway (open-source core or self-hosted commercial) as the egress point for all AI traffic outside the platform; the platform's own trust layer accepted for its native agents' reasoning. | Models are the fastest-changing and most price-volatile component. The switch between providers must stay in the company's hands, and spend must be visible in one place. |
| 2 · Control-plane layer | **Default to the platform.** Its control plane governs agents that live in and act on its estate. Keep the agent-network specification as the portable artifact; do not extend the plane to govern non-platform agents merely because it can register them. | Sharing rules, identity, and the data model already live there; policy fidelity is highest and integration cost lowest. The company is already dependent on the platform for this domain, so the marginal dependency is small. |
| 3 · Security layer | **Incumbent.** The existing security vendor's AI runtime protection; platform audit and event streams feed the SIEM. | Consolidation favours the CISO's current stack. Not a place to introduce a new vendor. |
| 4 · Oversight layer | **Independent by definition.** The AI inventory, risk classification, and evidence store span all vendors; every layer emits evidence to it. | A compliance system of record owned by one of the governed vendors cannot be the arbiter across the others. Platform trust dashboards are inputs, not the record. |
| 5 · Workforce layer | **Independent and owned.** A neutral layer governing the agents employees use across the platform, the productivity suite, and engineering tools, with the audit ledger inside the company's perimeter — the natural home of the open-source and source-available class. | The platform vendor has no answer here beyond its own collaboration tools; the productivity-suite vendor's bundle is the default gravity. The layer whose job is to preserve optionality should not belong to any platform being optioned. |

The design is deliberately asymmetric. It concedes the layer where the platform is strongest and the
company is already captive, and it holds the three layers whose function is to keep the company's
options open. Layer 3 is left with the incumbent because the security market is consolidating around
existing vendors and there is no advantage in fighting that.

### The case for defaulting to the platform

For a meaningful class of enterprises, defaulting to the platform is simply correct, and the
clearest way to present the choice is to start by describing that enterprise. Its AI ambitions are
largely confined to the platform's estate: the agents it wants are sales, service, and commerce
agents acting on platform data, not an enterprise-wide agent workforce. Its action volumes are
modest and will stay modest for the planning horizon. And its IT function is thin — there is no
platform-engineering team to spare for running independent infrastructure.

An enterprise that recognizes itself in that description should take the default, and the reasons
are substantial. Native governance has integration fidelity no third party can match: the platform's
policies understand its objects, sharing rules, and identity without translation, and its trust
layer is applied to its own agents' reasoning automatically. There is one vendor accountable, one
support relationship, one security review already completed, one set of compliance certifications
already vetted by procurement. The tooling arrives on an existing enterprise agreement rather than
through a new sourcing process. It is operated by administrators the company already employs, using
skills that are certifiable and hireable. And it rides a roadmap funded at a scale no independent
vendor approaches — the features missing today will very likely arrive. For this buyer, the
independent path is not prudence; it is an indulgence.

### The case against

The argument against turns on what governance is for. The governance layer is the one component
whose job is to let the enterprise change every other component — swap a model, retire an agent, add
a vendor, renegotiate a price — without re-governing from scratch. When the referee is owned by one
of the players, three things degrade together.

**Dependency deepens with every release.** Policies encoded in the vendor's policy format, agent
networks compiled into the vendor's runtime, audit history held in the vendor's hosted stores, and a
growing body of vendor-specific scripting and administrator skill all raise the cost of leaving —
and they raise it fastest precisely because the technology is evolving quickly. The surface area the
company depends on grows with each quarterly release, and roadmap timing becomes the vendor's
decision rather than the company's. Fast evolution cuts both ways: it delivers features quickly and
it deepens the well quickly.

**Reach is the second problem.** The vendor's plane governs the vendor's domain, so the company
still needs an independent layer for the productivity suite, for engineering, and for shadow AI —
which means "one vendor" becomes "one vendor plus," with the integration work the default was
supposed to avoid.

**Neutrality is the third.** A control plane whose registry, routing, and dashboards belong to the
vendor of the agents will make that vendor's agents the path of least resistance, and a governance
layer that meters consumption belongs to the same party that sells the consumption credits — a
structural conflict on the cost controls a board most cares about. Finally, evidence portability: an
audit trail held in a vendor's SaaS is available for as long as the relationship lasts and in the
form the vendor chooses, which is not the same as owning the record.

### Total cost of ownership

The default path is cheap to start and expensive to succeed with. Integration cost is near zero, but
the marginal cost of every governed action is set by consumption pricing — agent credits, data
credits, integration capacity — so total cost scales with adoption and the company holds no lever to
bring it down. The hidden entry cost is real where the integration platform is not already licensed:
the control plane presumes it, which can make a seven-figure prerequisite look like a governance
feature.

The independent path inverts the curve: higher initial cost in platform engineering, integration,
and skills, and a standing operations burden, against a fixed footprint plus raw inference with no
per-seat markup and the freedom to route work to a cheaper model when a cheaper model is good enough
— a lever worth a large fraction of inference cost at current price differentials between frontier
and second-tier models.

Over a three-to-five-year horizon the independent path tends to win where AI action volume is high
and growing, and the default tends to win where volume is modest and the estate is overwhelmingly
one platform. Either way, one discipline applies: token cost is not total cost of ownership. Both
paths carry evaluation, observability, human review, and incident management, and the independent
path's real cost is people, which should be priced explicitly rather than assumed away. Gartner
expects at least half of generative AI projects through 2028 to overrun their budgets for exactly
this reason.

### Future flexibility

The layers do not change at the same speed, and the design should follow the speed. Models change
fastest and, with frontier providers now clustered closely on capability, model choice is
increasingly a cost decision that should be re-made every few months — which is only possible if the
switch is independent. Agent frameworks and control planes change next fastest, and the protective
move is to insist on the open protocols the platforms themselves now support, MCP and A2A, as the
interfaces at every boundary, and to test portability claims rather than accept them: where a vendor
describes its agent-network specification as portable, export it and prove that it is. The workforce
layer changes slowest in principle but is the most exposed to bundling in practice, which is why it
should be owned.

The general rule is reversibility: default to the vendor where the company is already irreversibly
committed and the marginal dependency is small, and hold independent where the layer's purpose is
optionality. Where the company does default, the contract should carry the protections the
architecture cannot — export rights over policies and audit history in open formats, a standards
commitment on MCP and A2A, consumption price caps, and defined exit terms — because the cost of
leaving is set at signature, not at departure.

## 6. Conclusion

The AI governance market will look very different in three years. The gateways will be features of
clouds and security platforms; every system of record will ship a control plane; the oversight layer
will have integrated with all of them; and the workforce layer will either have a neutral standard
or will have been absorbed into the productivity suite.

What will not change is the shape of the problem. Enterprises will still need to decide where each
vendor's governance should stop, still need an operating model that graduates autonomy and measures
outcomes, still need the data, process, and evaluation work that no tool supplies, and still need
their people to adopt the tools in a way that surfaces value rather than hiding it. Those are
questions of judgment, and judgment is what an enterprise should be buying — from its own leadership
and from its advisors — before it buys any tool at all. The firms that will matter in this market
are not the ones with the best control plane. They are the ones that can tell a client, credibly and
against their own short-term interest, which control plane to use, where, and when to stop.

## Sources

Gartner, AI spending forecasts and generative/agentic AI project research (2024–2026); Gartner,
GenAI productivity survey (2025); McKinsey, *Seizing the Agentic AI Advantage* (2025) and *Rewired
to Outcompete* (2023); Stanford HAI, *AI Index* (2026); Salesforce Architects, *MuleSoft Agent
Fabric Deep Dive* (2026); Salesforce Agent Fabric announcements (Oct 2025, Apr 2026); Palo Alto
Networks, acquisition of Portkey (Jun 2026); HyperFRAME Research, analysis of the Portkey
acquisition (Jun 2026); Portkey, TrueFoundry, and Maxim AI (Bifrost) product documentation; Sirocco
Group, *Agent Fabric: The Real Test* (2026). Player lists are representative, not exhaustive;
positions as of September 2026.

## About the author

Roman Martynenko is the co-founder of Astound Digital, a global digital consultancy he helped build
from three founders to 1,500 people across twenty countries. Over twenty-six years with the firm he
has served as CFO, COO, and EVP of Corporate Development — leading its 2021 sale — and returned in
2026 as Chief Operating Officer to lead its rebuild as an AI-native business, with the firm itself
as the first client of every operating-model, pricing, and governance decision described in this
paper. His governance experience runs as deep as his operating experience: two decades as a director
of Astound, board and advisory roles with a portfolio of AI-native companies building the
infrastructure and operating models this paper describes, and credentialing as an NACD Certified
Director and NACD Board Leadership Fellow. He holds an MBA from INSEAD and degrees in economics and
business administration from UC Berkeley. The views expressed are his own.
