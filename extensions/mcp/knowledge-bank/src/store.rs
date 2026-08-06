//! In-memory document store seeded from bundled fixtures.
//!
//! The fixtures model the three source shapes a real knowledge bank
//! aggregates (workshop transcripts, Jira tickets, Confluence pages). Uploads
//! land in the same in-memory list, so within a server process an uploaded
//! document is immediately searchable; persistence is deliberately out of
//! scope for the stub.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

const FIXTURES: &str = include_str!("../fixtures/documents.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub doc_type: String,
    pub project: String,
    pub title: String,
    pub date: String,
    pub content: String,
}

#[derive(Debug)]
pub struct KnowledgeStore {
    documents: RwLock<Vec<Document>>,
}

impl KnowledgeStore {
    pub fn seeded() -> Result<Self, serde_json::Error> {
        let documents: Vec<Document> = serde_json::from_str(FIXTURES)?;
        Ok(Self {
            documents: RwLock::new(documents),
        })
    }

    pub fn list_documents(&self, project: Option<&str>, doc_type: Option<&str>) -> Vec<Document> {
        let documents = self
            .documents
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        documents
            .iter()
            .filter(|d| project.is_none_or(|p| d.project == p))
            .filter(|d| doc_type.is_none_or(|t| d.doc_type == t))
            .cloned()
            .collect()
    }

    pub fn search(&self, query: &str, project: Option<&str>, limit: usize) -> Vec<Document> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|t| t.len() > 2)
            .collect();
        let mut scored: Vec<(usize, Document)> = {
            let documents = self
                .documents
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            documents
                .iter()
                .filter(|d| project.is_none_or(|p| d.project == p))
                .filter_map(|d| {
                    let haystack = format!("{} {}", d.title, d.content).to_lowercase();
                    let score: usize = terms
                        .iter()
                        .map(|t| haystack.matches(t.as_str()).count())
                        .sum();
                    (score > 0).then(|| (score, d.clone()))
                })
                .collect()
        };
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored.into_iter().take(limit).map(|(_, d)| d).collect()
    }

    pub fn insert(&self, document: Document) {
        let mut documents = self
            .documents
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        documents.push(document);
    }

    pub fn count(&self) -> usize {
        self.documents
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}
