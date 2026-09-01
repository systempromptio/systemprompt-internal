//! # systemprompt-factsheet
//!
//! One template, one design system, many sheets.
//!
//! A factsheet is data — a [`FactsheetDoc`] of typed blocks — rendered through
//! a single Handlebars template and a single stylesheet. Sheets that ship with
//! the instance live as YAML under `storage/files/factsheet/sheets/`; a sheet
//! generated for a CRM lead is the same structure built in memory. There is no
//! difference between the two as far as this crate is concerned, which is the
//! whole point: a lead sheet is a new input, not new code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.

pub mod assets;
pub mod blocks;
pub mod error;
pub mod inline;
pub mod model;
pub mod render;
pub mod template_data;

pub use assets::Assets;
pub use blocks::{
    CompareRow, CompareSection, CtableCell, CtableDensity, CtableRow, CutPanel, FlowVariant,
    InvoicePart, InvoiceTone, LedgerField, LedgerRow, LedgerTone, Pill, ProvGroup, ProvRow,
    QboxItem, SpecRow, TrailKind, TrailStep, WhyItem,
};
pub use error::{FactsheetError, FactsheetResult};
pub use inline::{Inline, Span, SpanTone};
pub use model::{Block, FactsheetDoc, MetaItem, Page};
pub use render::{EnginePaths, FactsheetEngine, RenderedFactsheet};
