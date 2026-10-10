//! Tauri IPC commands. All heavy work happens on worker threads (Tauri async
//! commands run off the main thread automatically); UI stays responsive.

use crate::core::executor;
use crate::core::metadata::MetadataCache;
use crate::core::models::*;
use crate::core::preview::{build_preview, summarize};
use crate::core::scanner;
use crate::core::undo_store;
use parking_lot::RwLock;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub workspace: RwLock<Workspace>,
    pub metadata: Arc<MetadataCache>,
    pub cancel_flag: Arc<AtomicBool>,
    /// Active batch id currently being executed (journal protection)
    pub busy: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            workspace: RwLock::new(Workspace::default()),
            metadata: Arc::new(MetadataCache::new()),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            busy: AtomicBool::new(false),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub entries: Vec<FileEntry>,
    pub elapsed_ms: u64,
}

#[tauri::command]
pub async fn scan_paths(opts: ScanOptions, state: State<'_, AppState>) -> Result<ScanResult, String> {
    let started = std::time::Instant::now();
    let opts2 = opts.clone();
    let entries = tauri::async_runtime::spawn_blocking(move || scanner::scan(&opts2))
        .await
        .map_err(|e| e.to_string())??;

    let mut ws = state.workspace.write();
    ws.entries = entries.clone();
    drop(ws);

    Ok(ScanResult { elapsed_ms: started.elapsed().as_millis() as u64, entries })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub items: Vec<PlanItem>,
    pub total: usize,
    pub ready: usize,
    pub unchanged: usize,
    pub warnings: usize,
    pub errors: usize,
}

#[tauri::command]
pub async fn preview(state: State<'_, AppState>) -> Result<PreviewResult, String> {
    let ws = state.workspace.read().clone();
    let cache = state.metadata.clone();
    let items = tauri::async_runtime::spawn_blocking(move || build_preview(&ws, &cache))
        .await
        .map_err(|e| e.to_string())?;

    let summary = summarize(&items);
    Ok(PreviewResult {
        total: summary.total,
        ready: summary.ready,
        unchanged: summary.unchanged,
        warnings: summary.warnings,
        errors: summary.errors,
        items,
    })
}

#[tauri::command]
pub async fn update_rules(rules: Vec<crate::core::rules::RuleSpec>, state: State<'_, AppState>) -> Result<(), String> {
    state.workspace.write().rules = rules;
    Ok(())
}

#[tauri::command]
pub async fn set_conflict_strategy(strategy: ConflictStrategy, state: State<'_, AppState>) -> Result<(), String> {
    state.workspace.write().conflict_strategy = strategy;
    Ok(())
}

#[tauri::command]
pub async fn set_apply_to(apply_to: NamePart, state: State<'_, AppState>) -> Result<(), String> {
    state.workspace.write().apply_to = apply_to;
    Ok(())
}

#[tauri::command]
pub async fn set_manual_name(id: u64, name: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut ws = state.workspace.write();
    if let Some(e) = ws.entries.iter_mut().find(|e| e.id == id) {
        e.manual_name = name.filter(|n| !n.is_empty());
    }
    Ok(())
}

#[tauri::command]
pub async fn set_selection(ids: Vec<u64>, selected: bool, state: State<'_, AppState>) -> Result<(), String> {
    let mut ws = state.workspace.write();
    for e in ws.entries.iter_mut() {
        if ids.is_empty() || ids.binary_search(&e.id).is_ok() {
            e.selected = selected;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_entries(ids: Vec<u64>, state: State<'_, AppState>) -> Result<(), String> {
    let mut ws = state.workspace.write();
    ws.entries.retain(|e| !ids.contains(&e.id));
    Ok(())
}

#[tauri::command]
pub async fn clear_entries(state: State<'_, AppState>) -> Result<(), String> {
    state.workspace.write().entries.clear();
    Ok(())
}

#[tauri::command]
pub async fn apply_renames(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<ExecuteReport, String> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("another rename is already running".into());
    }
    state.cancel_flag.store(false, Ordering::SeqCst);
    log::info!("apply_renames requested");

    let ws = state.workspace.read().clone();
    let cache = state.metadata.clone();
    let cancel = state.cancel_flag.clone();
    let emitter = app.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        executor::execute_workspace(&ws, &cache, false, &|| cancel.load(Ordering::SeqCst), &|done, total| {
            // Push real-time progress to the UI every 50 items and at the end
            if done % 50 == 0 || done == total {
                use tauri::Emitter;
                let _ = emitter.emit("rename-progress", serde_json::json!({ "done": done, "total": total }));
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    state.busy.store(false, Ordering::SeqCst);

    let report = result?;
    log::info!(
        "apply_renames finished: renamed={} failed={} skipped={}",
        report.renamed,
        report.failed,
        report.skipped
    );
    if report.renamed > 0 {
        undo_store::record_batch(undo_store::HistoryRecord {
            batch_id: report.batch_id.clone(),
            created_at_ms: chrono::Utc::now().timestamp_millis(),
            renamed: report.renamed,
        });
        let _ = state.workspace.write();
    }
    Ok(report)
}

#[tauri::command]
pub async fn cancel_rename(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    log::info!("cancel_rename requested");
    Ok(())
}

#[tauri::command]
pub async fn undo_last() -> Result<String, String> {
    let batch_id = undo_store::last_batch_id().ok_or("nothing to undo")?;
    let (undone, skipped, errors) = executor::undo_batch(&batch_id)?;
    Ok(format!("undone={}, skipped={}, errors={:?}", undone, skipped, errors))
}

#[tauri::command]
pub async fn list_undo_history() -> Result<Vec<undo_store::HistoryRecord>, String> {
    Ok(undo_store::load_history())
}

/// Undo a specific batch by id (from the history panel). Newer batches that
/// renamed the same paths are automatically reverted first via journal replay,
/// so undoing batch N restores the exact state after batch N.
#[tauri::command]
pub async fn undo_batch(batch_id: String, state: State<'_, AppState>) -> Result<String, String> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("another rename is already running".into());
    }
    log::info!("undo_batch requested: {}", batch_id);
    let history = undo_store::load_history();
    let pos = history.iter().position(|r| r.batch_id == batch_id);
    let result = tauri::async_runtime::spawn_blocking(move || {
        // Revert batches made after `batch_id` (newest first) in effect, but
        // only undo the target; batches after it are simply undone, not replayed.
        match pos {
            Some(pos) => {
                // Undo newest -> target, so older batch paths are restored.
                for rec in history[pos..].iter().rev() {
                    let _ = executor::undo_batch(&rec.batch_id)?;
                }
                executor::undo_batch(&batch_id).map(|(undone, skipped, errors)| {
                    format!("undone={}, skipped={}, errors={:?}", undone, skipped, errors)
                })
            }
            None => Err(format!("unknown batch: {}", batch_id))
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    state.busy.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
pub async fn list_unfinished_batches() -> Result<Vec<JournalBatch>, String> {
    Ok(executor::unfinished_batches())
}

#[tauri::command]
pub async fn save_preset(name: String, rules: Vec<crate::core::rules::RuleSpec>) -> Result<(), String> {
    crate::cli::presets::save(&name, &rules)
}

#[tauri::command]
pub async fn load_preset(name: String) -> Result<Vec<crate::core::rules::RuleSpec>, String> {
    crate::cli::presets::load(&name)
}

#[tauri::command]
pub async fn list_presets() -> Result<Vec<String>, String> {
    crate::cli::presets::list()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportListData {
    /// [old_name, new_name] pairs (lines containing a comma)
    pub mapping: Vec<(String, String)>,
    /// Ordered new names (single-column lines), applied by working-set order
    pub ordered_names: Vec<String>,
}

/// Parse a CSV/TXT file for the Import List rule.
/// - `old,new` per line -> explicit mapping
/// - one name per line -> ordered list (applied in working-set order)
#[tauri::command]
pub async fn parse_import_list(path: String) -> Result<ImportListData, String> {
    log::info!("parse_import_list: {}", path);
    let content = tokio_or_blocking_read(&path).await?;
    let mut data = ImportListData { mapping: Vec::new(), ordered_names: Vec::new() };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((old, new)) = line.split_once(',') {
            let (old, new) = (old.trim(), new.trim());
            if !old.is_empty() && !new.is_empty() {
                data.mapping.push((old.to_string(), new.to_string()));
            }
        } else {
            data.ordered_names.push(line.to_string());
        }
    }
    log::info!(
        "parse_import_list: {} mappings, {} ordered names",
        data.mapping.len(),
        data.ordered_names.len()
    );
    Ok(data)
}

/// Read a file on a blocking thread (paths may be large).
async fn tokio_or_blocking_read(path: &str) -> Result<String, String> {
    let p = path.to_string();
    tauri::async_runtime::spawn_blocking(move || std::fs::read_to_string(&p))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("cannot read '{}': {}", path, e))
}

#[tauri::command]
pub async fn log_file_path() -> Result<String, String> {
    Ok(crate::core::logger::current_log_file().to_string_lossy().into_owned())
}
