//! Undo history persistence beyond the write-ahead journal: records batch ids
//! so the GUI/CLI can offer "undo last N batches" across restarts.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub batch_id: String,
    pub created_at_ms: i64,
    pub renamed: usize,
}

fn history_file() -> PathBuf {
    crate::core::executor::journal_dir().join("history.json")
}

pub fn load_history() -> Vec<HistoryRecord> {
    std::fs::read_to_string(history_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn record_batch(rec: HistoryRecord) {
    let mut all = load_history();
    all.push(rec);
    let _ = std::fs::create_dir_all(crate::core::executor::journal_dir());
    let _ = std::fs::write(history_file(), serde_json::to_string(&all).unwrap_or_default());
}

pub fn last_batch_id() -> Option<String> {
    load_history().last().map(|r| r.batch_id.clone())
}
