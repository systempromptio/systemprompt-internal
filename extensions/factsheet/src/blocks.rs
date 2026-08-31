//! The payload types the [`Block`](crate::model::Block) variants carry.
//!
//! Split from `model`, which keeps the document shape — doc, page, and the
//! block enum that dispatches to a template partial by its serde tag. These are
//! the leaves: one struct or enum per block's own data, each deriving
//! `JsonSchema` because a caller may build a sheet inline over the MCP wire.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::inline::Inline;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Glance {
    pub kicker: String,
    pub heading: String,
    pub cells: Vec<GlanceCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlanceCell {
    /// The large brand-coloured figure.
    pub n: String,
    /// The uppercase label beneath it.
    pub l: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapCard {
    pub kicker: String,
    pub heading: String,
    pub body: Inline,
    #[serde(default)]
    pub src: Option<String>,
    /// Peach card ground. Used to separate "what you get" cards from the
    /// white "what you are exposed to" cards.
    #[serde(default)]
    pub brand_tone: bool,
    #[serde(default)]
    pub pill: Option<Pill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PersonaCard {
    pub kicker: String,
    pub heading: String,
    pub body: Inline,
    pub outcome: String,
    /// The word before the outcome. "Result" unless the sheet says otherwise.
    #[serde(default = "default_outcome_label")]
    pub outcome_label: String,
}

fn default_outcome_label() -> String {
    "Result".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioCard {
    /// The timestamp or stage label, e.g. `09:00 · The email lands`.
    pub day: String,
    pub heading: String,
    pub body: Inline,
    #[serde(default)]
    pub brand_tone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TrailStep {
    /// The step's ordinal and name, e.g. `01 · SCOPE`.
    pub n: String,
    pub heading: String,
    pub body: Inline,
    #[serde(default)]
    pub kind: TrailKind,
}

/// What a trail step *is*, which decides how it is drawn: an ordinary step, one
/// of the gates that can refuse, or the terminal result.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrailKind {
    #[default]
    Plain,
    Gate,
    Result,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LedgerRow {
    pub fields: Vec<LedgerField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LedgerField {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub tone: LedgerTone,
    /// A parenthetical shown after the value, e.g. the reasons behind a
    /// verdict.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LedgerTone {
    #[default]
    Plain,
    /// An allowed decision.
    Ok,
    /// A refused decision.
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtableRow {
    pub label: Inline,
    #[serde(default)]
    pub cells: Vec<CtableCell>,
    /// A summing row: ruled off above and set bold.
    #[serde(default)]
    pub total: bool,
    /// A full-width banded row that captions the rows beneath it. `cells` is
    /// ignored; `label` spans the table.
    #[serde(default)]
    pub band: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtableCell {
    pub text: String,
    /// Render in brand colour: this is the cell the table exists to make.
    #[serde(default)]
    pub win: bool,
    /// Render in red: the number this sheet is arguing against.
    #[serde(default)]
    pub lose: bool,
    /// Set in a monospace face and never wrapped. True for most cells, because
    /// most cells in these tables are figures.
    #[serde(default = "default_true")]
    pub num: bool,
}

const fn default_true() -> bool {
    true
}

/// How tightly a comparison table is set. A four-column table breathes; an
/// eight-column one has to be compressed to stay on the page.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CtableDensity {
    #[default]
    Default,
    Tight,
    /// For a table nested inside a side-by-side cut.
    Mini,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CutPanel {
    pub title: String,
    pub headers: Vec<String>,
    pub rows: Vec<CtableRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvoicePart {
    pub text: String,
    #[serde(default)]
    pub tone: InvoiceTone,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InvoiceTone {
    #[default]
    Plain,
    /// The vendor or line name.
    Key,
    /// Detail the invoice states but does not explain.
    Dim,
    /// The figure being questioned.
    Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QboxItem {
    pub question: String,
    /// The parenthetical that tells the reader what the answer means.
    pub aside: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProvRow {
    /// A prose line, for a row that argues rather than lists.
    #[serde(default)]
    pub text: Option<Inline>,
    #[serde(default)]
    pub groups: Vec<ProvGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProvGroup {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WhyItem {
    pub heading: String,
    pub body: Inline,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpecRow {
    pub key: String,
    pub value: Inline,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompareSection {
    /// The banded heading above this group of rows.
    #[serde(default)]
    pub label: Option<String>,
    pub rows: Vec<CompareRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompareRow {
    pub label: String,
    pub cells: Vec<Inline>,
}

/// A short pill on a capability card, naming the standard or guarantee behind
/// the claim.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Pill {
    pub text: String,
    /// Set in a monospace face — for identifiers and RFC numbers.
    #[serde(default)]
    pub mono: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BentoHero {
    pub kicker: String,
    pub dim: String,
    pub big: String,
    pub body: Inline,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BentoTile {
    pub heading: String,
    pub body: Inline,
}

/// Where a flow caption sits, which decides which rule it carries. These were
/// inline `style=` overrides in the original sheets.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FlowVariant {
    /// Rule above, the default.
    #[default]
    Default,
    /// Directly beneath a diagram: rule below instead of above.
    UnderDiagram,
    /// Directly beneath a panel that already has a border: no rule at all.
    AfterPanel,
    /// As `after-panel`, on a page that needs the space back.
    AfterPanelTight,
}
