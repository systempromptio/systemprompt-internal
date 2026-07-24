//! Row and template types shared by the blog providers.

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug)]
pub(crate) struct BlogPost {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub image: Option<String>,
    pub category: Option<String>,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) struct RelatedPost {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReferenceLink {
    pub title: String,
    pub url: String,
}
