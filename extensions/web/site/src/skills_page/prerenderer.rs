use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt::extension::prelude::*;

use super::config::{SkillEntry, SkillsPageConfig};

#[derive(Debug)]
pub struct SkillsPagePrerenderer {
    config: Arc<SkillsPageConfig>,
}

impl SkillsPagePrerenderer {
    #[must_use]
    pub const fn new(config: Arc<SkillsPageConfig>) -> Self {
        Self { config }
    }
}

fn group_by_category(skills: &[SkillEntry]) -> Vec<serde_json::Value> {
    let mut grouped: BTreeMap<String, Vec<&SkillEntry>> = BTreeMap::new();
    for skill in skills {
        let category = skill
            .category
            .clone()
            .unwrap_or_else(|| "general".to_owned());
        grouped.entry(category).or_default().push(skill);
    }

    grouped
        .into_iter()
        .map(|(category, items)| {
            serde_json::json!({
                "name": category,
                "skills": items,
            })
        })
        .collect()
}

#[async_trait]
impl PagePrerenderer for SkillsPagePrerenderer {
    fn page_type(&self) -> &'static str {
        "skills-page"
    }

    fn priority(&self) -> u32 {
        50
    }

    async fn prepare(
        &self,
        ctx: &PagePrepareContext<'_>,
    ) -> Result<Option<PageRenderSpec>, systemprompt::traits::ProviderError> {
        let categories = group_by_category(&self.config.skills);

        let base_data = serde_json::json!({
            "site": ctx.web_config,
            "skills": {
                "items": self.config.skills,
                "categories": categories,
                "count": self.config.skills.len(),
            },
        });

        Ok(Some(PageRenderSpec::new(
            "skills",
            base_data,
            PathBuf::from("skills/index.html"),
        )))
    }
}
