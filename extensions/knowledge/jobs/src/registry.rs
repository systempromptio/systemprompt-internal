//! Inventory-derived job listing for the knowledge extension.
//!
//! Same pattern as the web extension's registry: jobs register themselves
//! with the global scheduler inventory via `submit_job!`, and
//! `Extension::jobs()` is derived from that inventory filtered by
//! [`JOB_TAG`], so there is no second hand-written list that can drift.

use std::sync::Arc;

use systemprompt::traits::{Job, JobContext, JobResult, ProviderError};

pub const JOB_TAG: &str = "knowledge-extension";

pub fn extension_jobs() -> Vec<Arc<dyn Job>> {
    let mut jobs: Vec<Arc<dyn Job>> = inventory::iter::<&'static dyn Job>()
        .filter(|job| job.tags().contains(&JOB_TAG))
        .map(|job| -> Arc<dyn Job> { Arc::new(StaticJob(*job)) })
        .collect();
    jobs.sort_by_key(|job| job.name());
    jobs
}

// Why: the inventory yields `&'static dyn Job` but the `Extension` trait
// wants owned `Arc<dyn Job>`; this zero-cost wrapper bridges the two.
struct StaticJob(&'static dyn Job);

#[async_trait::async_trait]
impl Job for StaticJob {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }

    fn schedule(&self) -> &'static str {
        self.0.schedule()
    }

    fn tags(&self) -> Vec<&'static str> {
        self.0.tags()
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult, ProviderError> {
        self.0.execute(ctx).await
    }

    fn enabled(&self) -> bool {
        self.0.enabled()
    }

    fn schedulable(&self) -> bool {
        self.0.schedulable()
    }
}
