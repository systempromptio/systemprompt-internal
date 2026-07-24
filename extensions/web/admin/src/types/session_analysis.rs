//! Shape of the AI session-analysis JSONB columns on `session_analyses`.
//!
//! One definition serves both ends: the tool-call schema the AI provider fills
//! in, and the JSONB columns the row is written to and read back from.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type SkillScores = HashMap<String, i16>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalOutcomeMapping {
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub achieved: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EfficiencyMetrics {
    #[serde(default)]
    pub total_turns: i32,
    #[serde(default)]
    pub duration_minutes: i32,
    #[serde(default)]
    pub corrections_count: i32,
    #[serde(default)]
    pub avg_turns_per_goal: f32,
    #[serde(default)]
    pub unnecessary_loops: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestPracticeItem {
    #[serde(default)]
    pub practice: String,
    #[serde(default)]
    pub score: String,
    #[serde(default)]
    pub note: String,
}
