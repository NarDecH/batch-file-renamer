//! Core data models shared by the engine, GUI commands and CLI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which part of the name a rule applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NamePart {
    #[default]
    /// Base name without extension
    Name,
    /// Extension without the dot
    Extension,
    /// Base name + "." + extension
    Full,
}

/// Status of a single planned rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    /// New name differs from old name and passed validation
    Ready,
    /// New name equals old name - nothing to do
    Unchanged,
    /// Validation warning (will still run, e.g. auto-suffix planned)
    Warning,
    /// Validation error - item will be skipped at execution
    Error,
    /// Manual override by the user in the preview table
    Manual,
}

/// One entry (file or folder) in the working set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Unique id within the session
    pub id: u64,
    /// Absolute path as scanned
    pub path: PathBuf,
    /// true = directory
    pub is_dir: bool,
    /// Final path component
    pub file_name: String,
    /// Size in bytes (0 for dirs)
    pub size: u64,
    /// mtime, epoch millis
    pub modified_ms: i64,
    /// ctime/birthtime, epoch millis (falls back to mtime)
    pub created_ms: i64,
    /// Whether the user ticked this row for renaming
    pub selected: bool,
    /// Manual override for the new name (applied after the pipeline)
    #[serde(default)]
    pub manual_name: Option<String>,
}

/// A planned rename for one entry: the outcome of the pipeline + validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: u64,
    pub old_name: String,
    /// Final new name for the selected part (base or base.ext depending on scope)
    pub new_name: String,
    /// Directory that contains the entry
    pub parent: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_ms: i64,
    pub status: ItemStatus,
    /// Short, user-facing message for warnings/errors
    #[serde(default)]
    pub message: Option<String>,
    /// Suggested fix applied automatically, e.g. "report (1).txt"
    #[serde(default)]
    pub resolved_name: Option<String>,
    pub selected: bool,
}

/// What to do when a new name collides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    /// Append " (1)", " (2)" ... before the extension
    AutoSuffix,
    /// Skip the conflicting item
    Skip,
    /// Mark as error; refuse to run until fixed
    Block,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        ConflictStrategy::Block
    }
}

/// Scope of the rename operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetScope {
    FilesOnly,
    DirsOnly,
    Both,
}

impl Default for TargetScope {
    fn default() -> Self {
        TargetScope::Both
    }
}

/// Options controlling the directory scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
    /// 0 = unlimited
    #[serde(default)]
    pub max_depth: u32,
    #[serde(default)]
    pub scope: TargetScope,
    /// Glob-style include filters, e.g. ["*.jpg", "*.png"]
    #[serde(default)]
    pub include_globs: Vec<String>,
    /// Glob-style exclude filters
    #[serde(default)]
    pub exclude_globs: Vec<String>,
    /// Regex include filter (matched against the file name)
    #[serde(default)]
    pub include_regex: Option<String>,
    /// Minimum size in bytes
    #[serde(default)]
    pub min_size: Option<u64>,
    /// Maximum size in bytes
    #[serde(default)]
    pub max_size: Option<u64>,
    /// mtime >= (epoch millis)
    #[serde(default)]
    pub modified_after_ms: Option<i64>,
    /// mtime <= (epoch millis)
    #[serde(default)]
    pub modified_before_ms: Option<i64>,
    #[serde(default)]
    pub include_hidden: bool,
    /// Never follow symlinks/junctions by default (infinite loop protection)
    #[serde(default = "default_true")]
    pub skip_symlinks: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            path: String::new(),
            recursive: true,
            max_depth: 0,
            scope: TargetScope::Both,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            include_regex: None,
            min_size: None,
            max_size: None,
            modified_after_ms: None,
            modified_before_ms: None,
            include_hidden: false,
            skip_symlinks: true,
        }
    }
}

/// Global engine state: the working set plus the rule pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub entries: Vec<FileEntry>,
    pub rules: Vec<crate::core::rules::RuleSpec>,
    /// Which part of the name the pipeline rewrites by default.
    /// Individual rules may override with their own `apply_to`.
    #[serde(default)]
    pub apply_to: NamePart,
    #[serde(default)]
    pub conflict_strategy: ConflictStrategy,
    /// Auto-numbering counters reset per parent directory
    #[serde(default)]
    pub number_reset_per_dir: bool,
}

/// Result summary for an executed rename batch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteReport {
    pub batch_id: String,
    pub renamed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failures: Vec<FailureItem>,
}

/// One failed rename inside a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureItem {
    pub from: String,
    pub to: String,
    pub error: String,
}

/// One entry in the write-ahead journal enabling crash-safe undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    /// Path before the rename
    pub from: String,
    /// Path after the rename
    pub to: String,
    /// true if this entry was a temporary intermediate name
    pub was_temp: bool,
    pub done: bool,
}

/// A complete batch record (journal file content).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalBatch {
    pub batch_id: String,
    pub created_at_ms: i64,
    pub entries: Vec<JournalEntry>,
    pub finished: bool,
}
