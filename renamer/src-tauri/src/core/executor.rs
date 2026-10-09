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
use rayon::prelude::*;
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

    // O(1) lookup set of pending sources: (parent, old_name) -> indices.
    // Replaces an O(n) scan per item (which made the whole loop O(n²)).
    let mut pending_sources: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, p) in plan.iter().enumerate() {
        pending_sources
            .entry((p.parent.clone(), p.old_name.clone()))
            .or_default()
            .push(i);
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

    // Write-ahead journal (JSONL: header line + one append line per entry)
    let journal_path = journal_file(&report.batch_id);
    let mut journal = JournalBatch {
        batch_id: report.batch_id.clone(),
        created_at_ms: now_ms(),
        entries: Vec::with_capacity(order.len()),
        finished: false,
    };
    // Header line only; entries are appended line-by-line below (O(1) each).
    std::fs::write(&journal_path, format!("{}\n", journal_header(&journal)?))
        .map_err(|e| e.to_string())?;
    let mut journal_flushed: usize = 0; // already-flushed entry count

    // Execute
    let mut done: HashMap<(String, String), PathBuf> = HashMap::new(); // (parent, old) -> current path
    let mut temp_final: Vec<(PathBuf, String)> = Vec::new(); // temp path -> final target name
    let mut executed: Vec<JournalEntry> = Vec::new();
    let mut failures: Vec<FailureItem> = Vec::new();
    // Index of entries not yet appended to the JSONL journal.
    let mut journal_dirty: usize = 0;

    // Split the work list into parallelizable items (no chain/cycle dependency,
    // destination is definitive) and sequential items (chains/cycles need the
    // "free the name first" ordering and temp-name handling).
    let mut parallel_idx: Vec<usize> = Vec::new();
    let mut serial_idx: Vec<usize> = Vec::new();
    for &idx in &order {
        let item = plan[idx];
        let target = item.resolved_name.clone().unwrap_or_else(|| item.new_name.clone());
        let claimed = pending_sources
            .get(&(item.parent.clone(), target.clone()))
            .is_some_and(|sources| sources.iter().any(|&ix| plan[ix].id != item.id));
        if !(ctx.cancelled)() && claimed {
            serial_idx.push(idx);
        } else if (ctx.cancelled)() {
            // Preserve the original behaviour: cancelled batches bail out below.
            serial_idx.push(idx);
        } else {
            parallel_idx.push(idx);
        }
    }

    // --- Parallel pass over independent items (owns its destination name) ---
    if !parallel_idx.is_empty() {
        let results: Vec<(usize, Result<(), String>)> = {
            let plan_ref = &plan;
            parallel_idx
                .par_iter()
                .map(|&idx| {
                    let item = plan_ref[idx];
                    let parent_path = PathBuf::from(&item.parent);
                    let from = parent_path.join(&item.old_name);
                    let target_name =
                        item.resolved_name.clone().unwrap_or_else(|| item.new_name.clone());
                    let to = parent_path.join(&target_name);
                    let outcome = if to.exists() && !same_file_ignoring_case(&from, &to) {
                        Err("destination already exists".to_string())
                    } else {
                        do_rename(&from, &to)
                    };
                    (idx, outcome)
                })
                .collect()
        };
        // Integrate results in stable order: journal, progress, accounting.
        for (progress_pos, (idx, outcome)) in results.into_iter().enumerate() {
            let item = plan[idx];
            let parent_path = PathBuf::from(&item.parent);
            let from = parent_path.join(&item.old_name);
            let target_name = item.resolved_name.clone().unwrap_or_else(|| item.new_name.clone());
            let to = parent_path.join(&target_name);
            match outcome {
                Ok(()) => {
                    log::debug!("renamed (parallel): {} -> {}", from.display(), to.display());
                    let entry = JournalEntry {
                        from: from.to_string_lossy().into_owned(),
                        to: to.to_string_lossy().into_owned(),
                        was_temp: false,
                        done: true,
                    };
                    executed.push(entry);
                    journal.entries.push(executed.last().unwrap().clone());
                    journal_dirty += 1;
                    if journal_dirty >= 20 {
                        let _ = append_journal(&journal_path, &journal, journal_flushed);
                        journal_flushed = journal.entries.len();
                    }
                    report.renamed += 1;
                    done.insert((item.parent.clone(), item.old_name.clone()), to.clone());
                }
                Err(e) => {
                    log::error!("rename failed: {} -> {}: {}", from.display(), to.display(), e);
                    failures.push(FailureItem {
                        from: from.to_string_lossy().into_owned(),
                        to: to.to_string_lossy().into_owned(),
                        error: e,
                    });
                    report.failed += 1;
                }
            }
            (ctx.progress)(progress_pos + 1, total);
        }
    }
    let _ = append_journal(&journal_path, &journal, journal_flushed);
    journal_flushed = journal.entries.len();
    // --- Sequential pass: chains/cycles need strict ordering + temp names ---
    let serial_total = parallel_idx.len() + serial_idx.len();
    for (progress_idx, &plan_idx) in serial_idx.iter().enumerate() {
        // Progress offset so serial items continue after the parallel pass.
        let progress_idx = progress_idx + parallel_idx.len();
        let _ = serial_total;
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
        // O(1) map lookup instead of an O(n) scan per item.
        let claimed_by_pending = pending_sources
            .get(&(item.parent.clone(), target_name.clone()))
            .is_some_and(|indices| indices.iter().any(|&ix| plan[ix].id != item.id));
        if to.exists()
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
                // Append-only JSONL: flush every 20 entries keeps crash-recovery
                // granularity while each flush costs only the new lines (O(delta)).
                journal_dirty += 1;
                if journal_dirty >= 20 {
                    let _ = append_journal(&journal_path, &journal, journal_flushed);
                    journal_flushed = journal.entries.len();
                    journal_dirty = 0;
                }
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
            }
            Err(e) => failures.push(FailureItem {
                from: temp_path.to_string_lossy().into_owned(),
                to: to_path.to_string_lossy().into_owned(),
                error: e,
            }),
        }
    }
    // Final flush ensures the journal on disk reflects all completed work even
    // when the batched threshold (20) was not hit exactly.
    let _ = append_journal(&journal_path, &journal, journal_flushed);

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
///
/// O(n) dependency-graph construction via index maps + Kahn's algorithm:
/// each item is emitted once and each edge relaxed once, instead of the
/// previous O(n³) triple-nested scan.
fn order_chains(
    plan: &[&PlanItem],
    order: &[usize],
    _rename_map: &HashMap<(String, String), String>,
) -> Vec<usize> {
    // Map (parent, old_name) -> positions in `plan` that still claim that name.
    let mut claimant: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, p) in plan.iter().enumerate() {
        claimant
            .entry((p.parent.clone(), p.old_name.clone()))
            .or_default()
            .push(i);
    }
    // Self-rename guard is unnecessary: a source name always leaves so it can
    // never be "blocked" by itself.

    // Edges: for each item A whose target is another pending item B's old
    // name (same dir), B must run before A  =>  edge B -> A.
    let mut indegree: HashMap<usize, usize> = HashMap::new();
    let mut dependents: HashMap<usize, Vec<usize>> = HashMap::new();
    for &idx in order {
        indegree.entry(idx).or_insert(0);
        let item = plan[idx];
        let target = item.resolved_name.clone().unwrap_or_else(|| item.new_name.clone());
        if let Some(sources) = claimant.get(&(item.parent.clone(), target.clone())) {
            for &b in sources {
                if b == idx {
                    continue;
                }
                // Edge b -> idx (b provides the name idx wants)
                let chain: &mut Vec<usize> = dependents.entry(b).or_default();
                if !chain.contains(&idx) {
                    chain.push(idx);
                    *indegree.entry(idx).or_insert(0) += 1;
                }
            }
        }
    }

    // Kahn's algorithm, seeded in the caller-provided order so stable input
    // order is preserved among independent items.
    let mut ready: std::collections::VecDeque<usize> = order
        .iter()
        .copied()
        .filter(|&idx| indegree.get(&idx).copied().unwrap_or(0) == 0)
        .collect();
    let mut result: Vec<usize> = Vec::with_capacity(order.len());
    while let Some(idx) = ready.pop_front() {
        result.push(idx);
        if let Some(deps) = dependents.get(&idx) {
            for &d in deps {
                let deg = indegree.get_mut(&d).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    ready.push_back(d);
                }
            }
        }
    }
    // Leftovers are pure cycles (>1 items mutually blocking); emit them in the
    // caller-provided order — the executor temp-names them.
    if result.len() < order.len() {
        let done: std::collections::HashSet<usize> = result.iter().copied().collect();
        for &idx in order {
            if !done.contains(&idx) {
                result.push(idx);
            }
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
            if let Ok(batch) = read_journal(&p) {
                if !batch.finished && !batch.entries.is_empty() {
                    out.push(batch);
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
    let mut batch = read_journal(&path)?;

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
    // Rewrite the JSONL with the updated header (finished=true); entries stay.
    let header = journal_header(&batch)?;
    let mut out = header;
    out.push('\n');
    for e in &batch.entries {
        out.push_str(&serde_json::to_string(e).unwrap_or_default());
        out.push('\n');
    }
    let _ = std::fs::write(&path, out);

    Ok((undone, skipped, errors))
}

/// Existing JSONL journal: read header + all appended entries.
fn read_journal(path: &Path) -> Result<JournalBatch, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = content.lines();
    let mut batch: JournalBatch = serde_json::from_str(
        lines.next().ok_or_else(|| "empty journal".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<JournalEntry>(line) {
            Ok(entry) => batch.entries.push(entry),
            // Torn tail from a crash mid-append: rest is unreliable, stop here.
            Err(_) => break,
        }
    }
    Ok(batch)
}

/// JSONL header line: batch metadata, entries appended separately.
fn journal_header(batch: &JournalBatch) -> Result<String, String> {
    let compact = JournalBatch {
        batch_id: batch.batch_id.clone(),
        created_at_ms: batch.created_at_ms,
        entries: Vec::new(),
        finished: batch.finished,
    };
    serde_json::to_string(&compact).map_err(|e| e.to_string())
}

/// Append new journal entries to the JSONL file (O(new entries), not O(all)).
fn append_journal(path: &Path, batch: &JournalBatch, from_index: usize) -> Result<(), String> {
    if from_index >= batch.entries.len() {
        return Ok(());
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    for e in &batch.entries[from_index..] {
        let line = serde_json::to_string(e).map_err(|e| e.to_string())?;
        writeln!(file, "{}", line).map_err(|e| e.to_string())?;
    }
    Ok(())
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
