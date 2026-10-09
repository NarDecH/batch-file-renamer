//! Preview builder: runs the pipeline over the working set and validates all
//! planned renames, using in-memory name sets instead of disk checks.

use crate::core::models::{ConflictStrategy, ItemStatus, NamePart, PlanItem, Workspace};
use crate::core::rules::pipeline::{apply_pipeline, validate_rule, RuleContext};
use crate::core::metadata::MetadataCache;
use crate::core::rules::regex_cache::RegexCache;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Compute the full preview for the workspace.
pub fn build_preview(ws: &Workspace, metadata_cache: &MetadataCache) -> Vec<PlanItem> {
    let started = std::time::Instant::now();
    // Compile every regex once per round (100k files must not recompile)
    let regex_cache = RegexCache::new();
    // Validate rules once per round (regex compile errors etc.)
    let mut rule_errors: HashMap<usize, String> = HashMap::new();
    for (i, rule) in ws.rules.iter().enumerate() {
        if let Err(e) = validate_rule(rule) {
            log::warn!("rule #{} invalid: {}", i + 1, e);
            rule_errors.insert(i, e);
        }
    }

    // Index context: position within the whole set and within each parent dir
    let mut index_in_dir: HashMap<u64, usize> = HashMap::new();
    let mut per_dir: HashMap<String, usize> = HashMap::new();
    for e in &ws.entries {
        let parent = parent_dir(&e.path);
        let counter = per_dir.entry(parent).or_insert(0);
        *counter += 1;
        index_in_dir.insert(e.id, *counter);
    }

    // Existing names per directory (for collision checks against the disk) -
    // built from the scanned set itself, so it covers the working set's dirs.
    // Case-insensitivity depends on the filesystem.
    let case_insensitive = cfg!(windows) || cfg!(target_os = "macos");

    let mut items: Vec<PlanItem> = Vec::with_capacity(ws.entries.len());

    for entry in &ws.entries {
        let mut base;
        let mut extension;
        // Split according to the workspace-level apply_to
        match ws.apply_to {
            NamePart::Extension => {
                base = String::new();
                extension = entry.file_name.rfind('.').filter(|&i| i > 0).map(|i| entry.file_name[i + 1..].to_string()).unwrap_or_default();
                let _ = &mut base;
            }
            _ => {
                let (b, e) = split_name_ext(&entry.file_name);
                base = b;
                extension = e;
            }
        }

        let parent_str = parent_dir(&entry.path);
        let ctx = RuleContext {
            index: (entry.id + 1) as usize,
            parent: &parent_str,
            path: &entry.path,
            size: entry.size,
            modified_ms: entry.modified_ms,
            created_ms: entry.created_ms,
            index_in_dir: index_in_dir[&entry.id],
            metadata: metadata_cache,
            regex_cache: &regex_cache,
            original_name: &entry.file_name,
        };

        if rule_errors.is_empty() {
            apply_pipeline(&ws.rules, ws.apply_to, &mut base, &mut extension, &ctx);
        }

        let new_full = assemble(&base, &extension, ws.apply_to, &entry.file_name);

        // Manual override wins
        let candidate = entry.manual_name.clone().unwrap_or(new_full);
        let status_kind = if entry.manual_name.is_some() { Some(ItemStatus::Manual) } else { None };

        items.push(PlanItem {
            id: entry.id,
            old_name: entry.file_name.clone(),
            new_name: candidate,
            parent: parent_str,
            is_dir: entry.is_dir,
            size: entry.size,
            modified_ms: entry.modified_ms,
            status: status_kind.unwrap_or(ItemStatus::Unchanged),
            message: None,
            resolved_name: None,
            selected: entry.selected,
        });
    }

    // Apply per-rule errors as item-level errors (any file affected by a broken rule)
    if !rule_errors.is_empty() {
        let msg = rule_errors.values().next().cloned().unwrap_or_default();
        for item in items.iter_mut() {
            item.status = ItemStatus::Error;
            item.message = Some(format!("rule error: {}", msg));
        }
        return items;
    }

    validate_and_resolve(ws, &mut items, case_insensitive);
    let s = summarize(&items);
    log::info!(
        "preview round: {} items, ready={}, unchanged={}, warn={}, errors={} in {:?} ({} rules)",
        s.total, s.ready, s.unchanged, s.warnings, s.errors, started.elapsed(), ws.rules.len()
    );
    items
}

/// Validate the planned names and apply the conflict strategy.
fn validate_and_resolve(ws: &Workspace, items: &mut [PlanItem], case_insensitive: bool) {
    // 1. Per-item static validation
    for item in items.iter_mut() {
        if item.new_name == item.old_name {
            item.status = ItemStatus::Unchanged;
            continue;
        }
        if let Some(err) = validate_new_name(&item.new_name, item.is_dir, case_insensitive, &item.old_name) {
            item.status = ItemStatus::Error;
            item.message = Some(err);
            continue;
        }
        item.status = ItemStatus::Ready;
    }

    // 2. Internal duplicates (new names colliding with each other or with
    //    unchanged existing names in the same directory).
    let mut seen: HashMap<(String, String), Vec<usize>> = HashMap::new(); // (dir, name) -> item indexes
    for (i, item) in items.iter().enumerate() {
        if matches!(item.status, ItemStatus::Error) {
            continue;
        }
        let key_name = if case_insensitive { item.new_name.to_lowercase() } else { item.new_name.clone() };
        seen.entry((item.parent.clone(), key_name)).or_default().push(i);
    }

    // Names of items that stay unchanged occupy their names permanently.
    let taken_by_unchanged: HashSet<(String, String)> = items
        .iter()
        .filter(|i| matches!(i.status, ItemStatus::Unchanged))
        .map(|i| {
            let name = if case_insensitive { i.old_name.to_lowercase() } else { i.old_name.clone() };
            (i.parent.clone(), name)
        })
        .collect();

    for (_, indexes) in seen {
        if indexes.len() < 2 {
            continue;
        }
        // Keep the first, resolve the rest per strategy
        for &idx in indexes.iter().skip(1) {
            resolve_conflict(ws.conflict_strategy, &mut items[idx], &taken_by_unchanged, case_insensitive);
        }
    }

    // 3. Collision of new names with unchanged names
    for item in items.iter_mut() {
        if matches!(item.status, ItemStatus::Ready | ItemStatus::Warning | ItemStatus::Manual) {
            let key_name = if case_insensitive { item.new_name.to_lowercase() } else { item.new_name.clone() };
            if taken_by_unchanged.contains(&(item.parent.clone(), key_name)) {
                resolve_conflict(ws.conflict_strategy, item, &taken_by_unchanged, case_insensitive);
            }
        }
    }
}

fn resolve_conflict(
    strategy: ConflictStrategy,
    item: &mut PlanItem,
    taken: &HashSet<(String, String)>,
    case_insensitive: bool,
) {
    match strategy {
        ConflictStrategy::Skip => {
            item.status = ItemStatus::Warning;
            item.message = Some("skipped: name conflict".into());
        }
        ConflictStrategy::Block => {
            item.status = ItemStatus::Error;
            item.message = Some("name conflict with another item".into());
        }
        ConflictStrategy::AutoSuffix => {
            let (base, ext) = split_name_ext(&item.new_name);
            for n in 1..10000u32 {
                let candidate = if ext.is_empty() {
                    format!("{} ({})", base, n)
                } else {
                    format!("{} ({}).{}", base, n, ext)
                };
                let key = if case_insensitive { candidate.to_lowercase() } else { candidate.clone() };
                if !taken.contains(&(item.parent.clone(), key)) {
                    item.resolved_name = Some(candidate.clone());
                    item.new_name = candidate;
                    item.status = ItemStatus::Warning;
                    item.message = Some("auto-suffixed due to conflict".into());
                    return;
                }
            }
            item.status = ItemStatus::Error;
            item.message = Some("could not find a free name".into());
        }
    }
}

/// Static per-name validation. Returns an error message when invalid.
fn validate_new_name(name: &str, is_dir: bool, case_insensitive: bool, old_name: &str) -> Option<String> {
    let _ = is_dir;
    if name.trim().is_empty() {
        return Some("new name is empty".into());
    }
    if name.len() > 255 {
        return Some("name exceeds 255 bytes".into());
    }
    for c in name.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            return Some(format!("forbidden character '{}'", c));
        }
        if c.is_control() {
            return Some("control character in name".into());
        }
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Some("name ends with dot or space".into());
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        return Some(format!("'{}' is a reserved Windows name", stem));
    }
    // Case-only rename must not be flagged as duplicate: caller ensures the
    // comparison here; when equal ignoring case but differing only by case we
    // allow it (the FS rename handles it on case-insensitive systems too).
    let _ = (case_insensitive, old_name);
    None
}

/// Split a file name into (base, extension-without-dot).
pub fn split_name_ext(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i + 1..].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn assemble(base: &str, extension: &str, apply_to: NamePart, original: &str) -> String {
    match apply_to {
        NamePart::Extension => {
            // base is empty in this mode; keep original base
            let (orig_base, _) = split_name_ext(original);
            if extension.is_empty() { orig_base } else { format!("{}.{}", orig_base, extension) }
        }
        NamePart::Name => {
            let (_, orig_ext) = split_name_ext(original);
            if orig_ext.is_empty() { base.to_string() } else { format!("{}.{}", base, orig_ext) }
        }
        NamePart::Full => {
            if extension.is_empty() { base.to_string() } else { format!("{}.{}", base, extension) }
        }
    }
}

fn parent_dir(path: &Path) -> String {
    path.parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Count summary for the status bar.
pub struct PreviewSummary {
    pub total: usize,
    pub ready: usize,
    pub unchanged: usize,
    pub warnings: usize,
    pub errors: usize,
}

pub fn summarize(items: &[PlanItem]) -> PreviewSummary {
    let mut s = PreviewSummary { total: items.len(), ready: 0, unchanged: 0, warnings: 0, errors: 0 };
    for i in items {
        match i.status {
            ItemStatus::Ready | ItemStatus::Manual => s.ready += 1,
            ItemStatus::Unchanged => s.unchanged += 1,
            ItemStatus::Warning => s.warnings += 1,
            ItemStatus::Error => s.errors += 1,
        }
    }
    s
}
