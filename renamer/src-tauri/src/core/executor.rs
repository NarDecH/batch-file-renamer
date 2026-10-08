//! Execution engine: performs the actual renames with cycle safety.
//!
//! Ordering rules:
//! - Deep directories first when folders are renamed (so children paths stay valid)
//! - Chains (A -> B where B is another item's old name) execute in dependency order
//! - Cycles (A -> B, B -> A) go through a temporary name
//! - Any failure rolls back completed steps within the same cycle group

use crate::core::models::{
    ExecuteReport, FailureItem, FileEntry, ItemStatus, JournalBatch, JournalEntry, PlanItem, Workspace,
};
use crate::core::preview::{build_preview, split_name_ext};
use crate::core::metadata::MetadataCache;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Context needed to execute a batch.
pub struct ExecuteContext<'a> {
    pub workspace: &'a Workspace,
    pub preview: &'a [PlanItem],
    pub metadata_cache: &'a MetadataCache,
    pub dry_run: bool,
    /// Cancellation flag checked between operations
    pub cancelled: &'a dyn Fn() -> bool,
    /// Progress callback: (done, total)
    pub progress: &'a dyn Fn(usize, usize),
}

/// Execute the planned renames. Only items with status Ready/Warning/Manual
/// that actually change the name are touched.
pub fn execute(ctx: &ExecuteContext) -> Result<ExecuteReport, String> {
    let plan: Vec<&PlanItem> = ctx
        .preview
        .iter()
        .filter(|p| {
            p.selected
                && matches!(p.status, ItemStatus::Ready | ItemStatus::Warning | ItemStatus::Manual)
                && p.new_name != p.old_name
        })
        .collect();

    let total = plan.len();
    let mut report = ExecuteReport::default();
    report.batch_id = uuid::Uuid::new_v4().to_string();

    if ctx.dry_run {
        report.renamed = total; // would-be renames
        log::info!("dry-run: {} renames would be performed", total);
        return Ok(report);
    }

    // Map (parent, old_name) -> new_name for chain/cycle analysis
    let mut rename_map: HashMap<(String, String), String> = HashMap::new();
    for p in &plan {
        rename_map.insert((p.parent.clone(), p.old_name.clone()), p.new_name.clone());
    }

    // Group by directory; directories last (rename files first, then folders,
    // deepest-first ordering handled per-directory below).
    let mut by_dir: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, p) in plan.iter().enumerate() {
        by_dir.entry(p.parent.clone()).or_default().push(i);
    }

    // Order directories deepest-first
    let mut dirs: Vec<&String> = by_dir.keys().collect();
    dirs.sort_by_key(|d| -(d.matches(['/', '\\']).count() as i64));

    // Build the full work list in execution order
    let mut order: Vec<usize> = Vec::with_capacity(plan.len());
    for dir in dirs {
        let mut idxs = by_dir[dir].clone();
        // Within a directory: files first (depth of path irrelevant), then dirs.
        // Chain order inside is resolved by dependency sort below.
        idxs.sort_by_key(|&i| plan[i].is_dir);
        order.extend(idxs);
    }

    // Dependency sort within each directory: item A's new name may equal item
    // B's old name; B must rename first (freeing the name) unless they form a cycle.
    order = order_chains(&plan, &order, &rename_map);

    log::info!(
        "execute: batch_id={} planned={} (dry_run={})",
        report.batch_id,
        total,
        ctx.dry_run
    );

    // Write-ahead journal
    let journal_path = journal_file(&report.batch_id);
    let mut journal = JournalBatch {
        batch_id: report.batch_id.clone(),
        created_at_ms: now_ms(),
        entries: Vec::with_capacity(order.len()),
        finished: false,
    };
    write_journal(&journal_path, &journal)?;

    // Execute
    let mut done: HashMap<(String, String), PathBuf> = HashMap::new(); // (parent, old) -> current path
    let mut temp_final: Vec<(PathBuf, String)> = Vec::new(); // temp path -> final target name
    let mut executed: Vec<JournalEntry> = Vec::new();
    let mut failures: Vec<FailureItem> = Vec::new();

    for (progress_idx, &plan_idx) in order.iter().enumerate() {
        if (ctx.cancelled)() {
            // Roll back what was done so far so nothing is left half-renamed
            rollback(&executed);
            report.renamed = 0;
            report.skipped = 0;
            report.failed = executed.len();
            report.failures.push(FailureItem {
                from: "(batch)".into(),
                to: "(batch)".into(),
                error: "cancelled by user; all renames reverted".into(),
            });
            finalize_journal(&journal_path, &journal, executed);
            return Ok(report);
        }

        let item = plan[plan_idx];
        let parent_path = PathBuf::from(&item.parent);
        let from = parent_path.join(&item.old_name);
        let target_name = item.resolved_name.clone().unwrap_or_else(|| item.new_name.clone());
        let to = parent_path.join(&target_name);

        // Final on-disk collision check. A destination is allowed when it is
        // (a) the source's own case-only variant, or (b) the old name of another
        // pending item in this batch (chain/cycle) - that item moves away first
        // or we handle the overlap with a temp name below.
        let claimed_by_pending = plan.iter().any(|other| {
            !std::ptr::eq(*other, item)
                && other.parent == item.parent
                && other.old_name == target_name
        });            if to.exists()
            && !same_file_ignoring_case(&from, &to)
            && !claimed_by_pending
        {
            log::warn!("blocked: destination already exists: {}", to.display());
            failures.push(FailureItem {
                from: from.to_string_lossy().into_owned(),
                to: to.to_string_lossy().into_owned(),
                error: "destination already exists".into(),
            });
            report.failed += 1;
            (ctx.progress)(progress_idx + 1, total);
            continue;
        }

        // Cycle detection: if `to` is another item's pending source path and
        // that item has not run yet, use a temporary name.
        let needs_temp = claimed_by_pending
            && !done.contains_key(&(item.parent.clone(), target_name.clone()));

        let (actual_to, was_temp) = if needs_temp {
            let (base, ext) = split_name_ext(&target_name);
            let temp = format!(
                "{}__rntmp__{}{}",
                base,
                uuid::Uuid::new_v4().simple(),
                if ext.is_empty() { String::new() } else { format!(".{}", ext) }
            );
            (parent_path.join(&temp), true)
        } else {
            (to.clone(), false)
        };

        match do_rename(&from, &actual_to) {
            Ok(()) => {
                log::debug!(
                    "renamed{}: {} -> {}",
                    if was_temp { " (temp)" } else { "" },
                    from.display(),
                    actual_to.display()
                );
                let entry = JournalEntry {
                    from: from.to_string_lossy().into_owned(),
                    to: actual_to.to_string_lossy().into_owned(),
                    was_temp,
                    done: true,
                };
                executed.push(entry);
                journal.entries.push(executed.last().unwrap().clone());
                let _ = write_journal(&journal_path, &journal);
                report.renamed += 1;
                done.insert((item.parent.clone(), item.old_name.clone()), actual_to.clone());
                if was_temp {
                    temp_final.push((actual_to.clone(), target_name.clone()));
                }
            }
            Err(e) => {
                log::error!("rename failed: {} -> {}: {}", from.display(), actual_to.display(), e);
                failures.push(FailureItem {
                    from: from.to_string_lossy().into_owned(),
                    to: actual_to.to_string_lossy().into_owned(),
                    error: e,
                });
                report.failed += 1;
                // If this item was part of a chain/cycle, roll back its group
                rollback_group(&plan, &executed, &mut report);
                break;
            }
        }
        (ctx.progress)(progress_idx + 1, total);
    }

    // Second pass: rename temp-named items to their final names
    for (temp_path, final_name) in temp_final {
        let parent = temp_path.parent().unwrap_or(Path::new("")).to_path_buf();
        let to_path = parent.join(&final_name);
        match do_rename(&temp_path, &to_path) {
            Ok(()) => {
                let e2 = JournalEntry {
                    from: temp_path.to_string_lossy().into_owned(),
                    to: to_path.to_string_lossy().into_owned(),
                    was_temp: false,
                    done: true,
                };
                journal.entries.push(e2.clone());
                executed.push(e2);
                let _ = write_journal(&journal_path, &journal);
            }
            Err(e) => failures.push(FailureItem {
                from: temp_path.to_string_lossy().into_owned(),
                to: to_path.to_string_lossy().into_owned(),
                error: e,
            }),
        }
    }

    report.failures.extend(failures);
    journal.finished = true;
    finalize_journal(&journal_path, &journal, executed);
    Ok(report)
}

fn do_rename(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|e| e.to_string())
}

/// Order items so that when A's target name is B's current name, B goes first -
/// unless A -> B and B -> A (cycle), in which case order is arbitrary and the
/// executor uses temp names.
fn order_chains(
    plan: &[&PlanItem],
    order: &[usize],
    _rename_map: &HashMap<(String, String), String>,
) -> Vec<usize> {
    // Simple greedy: repeatedly pick an item whose target name is not blocked
    // by another pending item's old name in the same directory.
    let mut remaining: Vec<usize> = order.to_vec();
    let mut result = Vec::with_capacity(order.len());
    let pending: std::collections::HashSet<(String, String)> = plan
        .iter()
        .map(|p| (p.parent.clone(), p.old_name.clone()))
        .collect();

    while !remaining.is_empty() {
        let mut progressed = false;
        let mut i = 0;
        while i < remaining.len() {
            let idx = remaining[i];
            let item = plan[idx];
            let target = item.resolved_name.clone().unwrap_or_else(|| item.new_name.clone());
            let blocked = plan.iter().any(|other| {
                other.parent == item.parent
                    && pending.contains(&(item.parent.clone(), target.clone()))
                    && remaining.iter().any(|&r| {
                        plan[r].parent == other.parent && plan[r].old_name == target && r != idx
                    })
            });
            if blocked {
                i += 1;
            } else {
                result.push(idx);
                remaining.remove(i);
                progressed = true;
            }
        }
        if !progressed {
            // Pure cycle: emit remaining in original order; executor temp-names them
            result.extend(remaining.drain(..));
        }
    }
    result
}

fn rollback(executed: &[JournalEntry]) {
    for e in executed.iter().rev() {
        let _ = std::fs::rename(&e.to, &e.from);
    }
}

fn rollback_group(plan: &[&PlanItem], executed: &[JournalEntry], report: &mut ExecuteReport) {
    // Roll back entries whose (parent) participates in a chain/cycle with the
    // failed item. Simplified: roll back everything in dirs that had a temp.
    let temp_dirs: std::collections::HashSet<String> = executed
        .iter()
        .filter(|e| e.was_temp)
        .map(|e| Path::new(&e.to).parent().unwrap_or(Path::new("")).to_string_lossy().into_owned())
        .collect();
    for e in executed.iter().rev() {
        let dir = Path::new(&e.to).parent().unwrap_or(Path::new("")).to_string_lossy().into_owned();
        if temp_dirs.contains(&dir) {
            let _ = std::fs::rename(&e.to, &e.from);
            report.renamed = report.renamed.saturating_sub(1);
        }
    }
    let _ = plan;
}

fn same_file_ignoring_case(a: &Path, b: &Path) -> bool {
    if cfg!(windows) || cfg!(target_os = "macos") {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

fn strip_temp_suffix(name: &str) -> (String, String) {
    if let Some(i) = name.find("__rntmp__") {
        (name[..i].to_string(), name[i + 9..].to_string())
    } else {
        (name.to_string(), String::new())
    }
}

fn temp_suffix_of(name: &str) -> String {
    // The temp name embeds a uuid; recover the original by checking the plan is
    // done by the caller. Return the name itself for plan lookup fallback.
    name.to_string()
}

/// Directory for journal files: <config>/batch-renamer/journal
pub fn journal_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("batch-renamer").join("journal")
}

pub fn journal_file(batch_id: &str) -> PathBuf {
    journal_dir().join(format!("{}.json", batch_id))
}

/// List unfinished batches (for crash recovery / cross-restart undo).
pub fn unfinished_batches() -> Vec<JournalBatch> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(journal_dir()) else { return out };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(batch) = serde_json::from_str::<JournalBatch>(&content) {
                    if !batch.finished && !batch.entries.is_empty() {
                        out.push(batch);
                    }
                }
            }
        }
    }
    out.sort_by_key(|b| b.created_at_ms);
    out
}

/// Undo the most recent (or given) batch. Returns (undone, skipped, errors).
pub fn undo_batch(batch_id: &str) -> Result<(usize, usize, Vec<String>), String> {
    log::info!("undo start: batch={}", batch_id);
    let path = journal_file(batch_id);
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut batch: JournalBatch = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mut undone = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for entry in batch.entries.iter().rev() {
        if !entry.done {
            skipped += 1;
            continue;
        }
        let from = PathBuf::from(&entry.to);
        let to = PathBuf::from(&entry.from);
        if !from.exists() {
            // Maybe a temp entry already got its final rename; skip silently
            skipped += 1;
            continue;
        }
        // Verify the file is still the one we renamed (basic sanity)
        if to.exists() && !same_file_ignoring_case(&from, &to) {
            errors.push(format!("undo target exists, skipping: {}", to.display()));
            skipped += 1;
            continue;
        }
        match std::fs::rename(&from, &to) {
            Ok(()) => {
                log::info!("undo: {} -> {}", from.display(), to.display());
                undone += 1;
            }
            Err(e) => {
                log::error!("undo failed: {}: {}", from.display(), e);
                errors.push(format!("{}: {}", from.display(), e));
            }
        }
    }

    log::info!(
        "undo batch {} finished: undone={} skipped={} errors={}",
        batch_id,
        undone,
        skipped,
        errors.len()
    );

    // Mark as finished so it is not re-offered for undo after a crash
    batch.finished = true;
    let _ = std::fs::write(&path, serde_json::to_string(&batch).unwrap_or_default());

    Ok((undone, skipped, errors))
}

/// Write the current journal state to disk (write-ahead).
fn write_journal(path: &Path, batch: &JournalBatch) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string(batch).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn finalize_journal(_path: &Path, _batch: &JournalBatch, _executed: Vec<JournalEntry>) {
    // Journal already persisted incrementally in write_journal; nothing to do.
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Rebuild preview + execute in one call (used by CLI).
pub fn execute_workspace(
    ws: &Workspace,
    metadata_cache: &MetadataCache,
    dry_run: bool,
    cancelled: &dyn Fn() -> bool,
    progress: &dyn Fn(usize, usize),
) -> Result<ExecuteReport, String> {
    let preview = build_preview(ws, metadata_cache);
    let ctx = ExecuteContext {
        workspace: ws,
        preview: &preview,
        metadata_cache,
        dry_run,
        cancelled,
        progress,
    };
    execute(&ctx)
}

/// Export an execute report to CSV.
pub fn export_report_csv(report: &ExecuteReport, path: &Path) -> Result<(), String> {
    let mut out = String::from("from,to,error\n");
    for f in &report.failures {
        out.push_str(&format!("{:?},{:?},{}\n", f.from, f.to, f.error.replace(',', ";")));
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// Helper used by tests/GUI to construct FileEntry quickly.
pub fn entry_from_path(id: u64, path: PathBuf, is_dir: bool) -> FileEntry {
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    FileEntry {
        id,
        path,
        is_dir,
        file_name,
        size: 0,
        modified_ms: 0,
        created_ms: 0,
        selected: true,
        manual_name: None,
    }
}
