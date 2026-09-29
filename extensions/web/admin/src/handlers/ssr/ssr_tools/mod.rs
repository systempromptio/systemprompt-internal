//! The Tools and Artifacts pages' shared row helpers.
//!
//! Only the kind glyph, kind label and payload-size formatting are here: the
//! conversation detail page draws its tool and artifact rows with them. The
//! pages themselves (`/admin/tools`, `/admin/artifacts`) land with the rest of
//! this module.

pub(crate) mod rows;
