//! Page-window arithmetic for paginated list views.

// Why: one bundle for the five numbers a footer needs, so each page's
// pagination builder takes a window rather than a row of bare integers.
#[derive(Debug, Clone, Copy)]
pub struct PageWindow {
    // Why: Zero-based page index.
    pub index: i64,
    pub size: i64,
    pub total_pages: i64,
    pub total_rows: i64,
    // Why: Rows actually rendered on this page — the last page is short.
    pub shown_rows: i64,
    // Why: What the rows are called: "Showing 1-50 of 54 <noun>".
    pub noun: &'static str,
}

impl PageWindow {
    // Why: an empty result still renders as "page 1 of 1" rather than "of 0",
    // which is the one case the ceiling division cannot produce on its own.
    pub const fn new(
        index: i64,
        size: i64,
        total_rows: i64,
        shown_rows: i64,
        noun: &'static str,
    ) -> Self {
        let total_pages = if total_rows == 0 {
            1
        } else {
            (total_rows + size - 1) / size
        };
        Self {
            index,
            size,
            total_pages,
            total_rows,
            shown_rows,
            noun,
        }
    }

    // Why: The 1-based inclusive row range this page covers, `(0, 0)` when empty.
    pub const fn bounds(self) -> (i64, i64) {
        if self.shown_rows == 0 {
            return (0, 0);
        }
        let first = self.index * self.size + 1;
        (first, self.index * self.size + self.shown_rows)
    }
}
