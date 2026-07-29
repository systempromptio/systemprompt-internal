//! Template context types for the SSR pages.

pub(crate) mod charts;
mod settings;
mod users;

pub(crate) use charts::{ChartView, HistogramView, bar_pct};
pub(crate) use settings::*;
pub(crate) use users::*;
