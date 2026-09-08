//! Template context types for the SSR pages.

mod budget;
mod charts;
mod enterprises;
mod settings;
mod tabs;
mod users;

pub(crate) use budget::*;
pub(crate) use charts::*;
pub(crate) use enterprises::*;
pub(crate) use settings::*;
pub(crate) use tabs::*;
pub(crate) use users::*;

mod pie;
pub(crate) use pie::*;

mod svg_line;
pub(crate) use svg_line::*;

mod svg_stack;
pub(crate) use svg_stack::*;

mod groups;
pub(crate) use groups::*;

mod groups_listing;
pub(crate) use groups_listing::*;

mod projects_page;
pub(crate) use projects_page::*;

mod breadcrumb;
pub(crate) use breadcrumb::*;

mod table;
pub(crate) use table::*;

mod charts_daily;
pub(crate) use charts_daily::*;
