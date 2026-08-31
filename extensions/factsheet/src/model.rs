//! The factsheet content model.
//!
//! A sheet is a sequence of typed blocks, not a hand-authored HTML file. Every
//! block variant corresponds to one partial in
//! `storage/files/factsheet/templates/partials/`, named by its serde tag — the
//! template dispatches on that tag, so adding a block type means adding a
//! variant here and a partial of the same name, and nothing else.
//!
//! Hand-tuned inline `style=` overrides are deliberately absent. Where the
//! original sheets carried one, it is expressed as a modifier on the block (see
//! [`FlowVariant`]) and lives in `base.css` with the rest of the design system.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::inline::Inline;

/// A sheet's two-page budget. The house style is a one- or two-page document;
/// anything longer is a formatting failure, not a longer factsheet.
pub const DEFAULT_MAX_PAGES: usize = 2;

const fn default_max_pages() -> usize {
    DEFAULT_MAX_PAGES
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FactsheetDoc {
    pub id: String,
    /// Document `<title>`. Never rendered on the page itself.
    pub title: String,
    /// Running footer, printed bottom-left on every page via the `@page`
    /// margin box. One field, because the original pipeline defined this twice
    /// — once in CSS and once in a Chromium footer template — and they drifted.
    pub footer: String,
    /// Filename under `assets/`, resolved for any `diagram` block.
    #[serde(default)]
    pub diagram: Option<String>,
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
    /// Per-sheet CSS delta. Keep it small; anything reusable belongs in
    /// `base.css`.
    #[serde(default)]
    pub variant_css: Option<String>,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Page {
    /// Distribute blocks over the full printable height instead of letting them
    /// bunch at the top. Page 1 sets this; continuation pages do not.
    #[serde(default)]
    pub fill: bool,
    /// Tighten the shared vertical rhythm. The technical sheets carry more
    /// blocks per page than the executive ones and need the space back; this is
    /// a density control on the design system, not a per-sheet override.
    #[serde(default)]
    pub dense: bool,
    pub masthead: Vec<MetaItem>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MetaItem {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Block {
    HeroRow {
        kicker: String,
        headline: Inline,
        lede: Inline,
        /// A second lede paragraph. Rare, and a sign the opening is doing a lot
        /// of work — but the partner sheet genuinely needs two beats.
        #[serde(default)]
        extra_lede: Option<Inline>,
        #[serde(default)]
        glance: Option<Glance>,
    },
    Pov {
        kicker: String,
        statement: Inline,
        #[serde(default)]
        src: Option<String>,
    },
    SecHead {
        title: String,
        /// Assigned in document order at render time. Numbering by hand is how
        /// a sheet ends up with two section 03s after an edit.
        #[serde(skip_deserializing, default)]
        num: Option<String>,
    },
    Caps {
        cards: Vec<CapCard>,
    },
    Personas {
        cards: Vec<PersonaCard>,
        /// Tighten the gap below. Sheets that follow the strip with another
        /// block need it; sheets that end on it do not.
        #[serde(default)]
        spaced: bool,
    },
    /// A three-step narrative strip — the same moment seen from two positions,
    /// with the third card carrying the brand ground because it is the one the
    /// sheet is arguing for.
    Scenario {
        cards: Vec<ScenarioCard>,
    },
    Prov {
        kicker: String,
        rows: Vec<ProvRow>,
        #[serde(default)]
        spaced: bool,
    },
    Bento4 {
        hero: BentoHero,
        /// Exactly two tiles sit on the top row; a third spans the width below.
        top: Vec<BentoTile>,
        wide: BentoTile,
    },
    FlowCaption {
        label: String,
        text: String,
        #[serde(default)]
        variant: FlowVariant,
    },
    /// A comparison table. `label` is the row's stub; every other cell is
    /// numeric and set in a monospace face.
    Ctable {
        headers: Vec<String>,
        rows: Vec<CtableRow>,
        #[serde(default)]
        density: CtableDensity,
    },
    /// Two tables side by side, cutting the same total two ways.
    Cuts {
        panels: Vec<CutPanel>,
    },
    /// The single opaque vendor line, quoted as it arrives.
    Invoice {
        comment: String,
        parts: Vec<InvoicePart>,
        #[serde(default)]
        footnote: Option<String>,
    },
    /// The spend that never reached the invoice. Deliberately the only red
    /// surface in the house style.
    Offbook {
        kicker: String,
        body: Inline,
    },
    /// A brand-toned panel that fills its column, for the one thing a sheet
    /// wants the reader to picture rather than parse.
    Callout {
        kicker: String,
        lead: Inline,
        body: Inline,
    },
    /// A numbered box of questions, each with a muted parenthetical aside. The
    /// house device for "take this to your CFO".
    Qbox {
        #[serde(default)]
        kicker: Option<String>,
        /// Numbered questions. Empty when the box carries prose instead.
        #[serde(default)]
        items: Vec<QboxItem>,
        /// Prose paragraphs, for when the box makes an argument rather than
        /// asking a set of questions.
        #[serde(default)]
        paragraphs: Vec<Inline>,
    },
    /// A single large pull-quote. Structurally a point-of-view band carrying one
    /// line rather than a kicker and a claim.
    Quote {
        text: Inline,
    },
    /// A left-to-right walkthrough of one pipeline, arrows drawn between the
    /// steps. Used to show what happens to a single action.
    Trail {
        steps: Vec<TrailStep>,
    },
    /// A dark monospace ledger — what the audit rows actually look like. Modelled
    /// as key/value fields rather than free text so a row cannot be typed into
    /// something the system would never emit.
    Ledger {
        #[serde(default)]
        comment: Option<String>,
        rows: Vec<LedgerRow>,
    },
    /// Two columns side by side, each holding its own blocks. The technical
    /// sheets set a rationale list against a specification table this way.
    TwoCol {
        left: Vec<Self>,
        right: Vec<Self>,
    },
    /// An arrow-led list of reasons. Longer-form than a capability card,
    /// because each item argues rather than states.
    WhyList {
        items: Vec<WhyItem>,
    },
    /// A specification table: a label column and a dense value column.
    Spec {
        rows: Vec<SpecRow>,
    },
    /// A competitive comparison. One column is ours and is highlighted; rows
    /// are grouped under banded section labels.
    Compare {
        /// Column headings, excluding the leading stub column.
        columns: Vec<String>,
        /// Index into `columns` of the column that is ours.
        #[serde(default)]
        highlight: Option<usize>,
        sections: Vec<CompareSection>,
    },
    Diagram,
    Cta {
        headline: Inline,
        body: Inline,
        #[serde(default)]
        email: Option<String>,
        #[serde(default)]
        link: Option<String>,
    },
}

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
    /// A parenthetical shown after the value, e.g. the reasons behind a verdict.
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


impl FactsheetDoc {
    /// Assign section numbers in document order, across pages.
    pub fn number_sections(&mut self) {
        let mut n: u32 = 0;
        for page in &mut self.pages {
            number_blocks(&mut page.blocks, &mut n);
        }
    }
}

fn number_blocks(blocks: &mut [Block], n: &mut u32) {
    for block in blocks {
        match block {
            Block::SecHead { num, .. } => {
                *n += 1;
                *num = Some(format!("{n:02}"));
            },
            Block::TwoCol { left, right } => {
                number_blocks(left, n);
                number_blocks(right, n);
            },
            _ => {},
        }
    }
}
