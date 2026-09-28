//! The Analysis section's shared pieces. Only the window-bound parser lives
//! here so far — the export dialog's window resolution reads it — and the
//! `/admin/analysis/<noun>` pages join it when the analysis suite lands.

pub(crate) mod time;
