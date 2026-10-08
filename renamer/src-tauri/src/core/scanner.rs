//! Parallel directory scanning.
//!
//! Uses `jwalk` for parallel traversal (sorted off, filters applied inline) and
//! never follows symlinks/junctions by default to avoid infinite loops.

use super::models::{FileEntry, ScanOptions, TargetScope};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Scan `opts.path` and return the filtered entries sorted by natural order of
/// the full path. Runs on a worker thread; the caller keeps the UI responsive.
pub fn scan(opts: &ScanOptions) -> Result<Vec<FileEntry>, String> {
    let started = std::time::Instant::now();
    log::info!(
        "scan start: path='{}' recursive={} max_depth={} scope={:?} include={:?} exclude={:?} hidden={}",
        opts.path, opts.recursive, opts.max_depth, opts.scope,
        opts.include_globs, opts.exclude_globs, opts.include_hidden
    );
    let root = Path::new(&opts.path);
    if !root.exists() {
        log::error!("scan failed: path does not exist: {}", opts.path);
        return Err(format!("path does not exist: {}", opts.path));
    }

    let include_matcher = build_glob_set(&opts.include_globs)?;
    let exclude_matcher = build_glob_set(&opts.exclude_globs)?;
    let include_re = match &opts.include_regex {
        Some(p) => Some(
            regex::Regex::new(p)
                .map_err(|e| format!("invalid include regex: {}", e))?,
        ),
        None => None,
    };

    let walk_depth = if opts.recursive && opts.max_depth > 0 {
        opts.max_depth as usize
    } else if opts.recursive {
        usize::MAX
    } else {
        1
    };

    let mut entries: Vec<FileEntry> = Vec::new();
    let walker = jwalk::WalkDir::new(root)
        .min_depth(1)
        .max_depth(walk_depth)
        .follow_links(false)
        .sort(false)
        .process_read_dir({
            let opts = opts.clone();
            let include_matcher = include_matcher.clone();
            let exclude_matcher = exclude_matcher.clone();
            let include_re = include_re.clone();
            move |_depth, _path, _read_dir_state, children| {
                children.retain(|child| {
                    let Ok(dir_entry) = child.as_ref() else {
                        return false;
                    };
                    if dir_entry.file_type().is_symlink() && opts.skip_symlinks {
                        return false;
                    }
                    let is_dir = dir_entry.file_type().is_dir();
                    // Keep directories for traversal unless they are excluded by globs
                    let name = dir_entry.file_name().to_string_lossy().to_string();
                    if is_dir {
                        if name.starts_with('.') && !opts.include_hidden {
                            return false;
                        }
                        if exclude_matcher.as_ref().is_some_and(|m| m.is_match(&name)) {
                            return false;
                        }
                        return true;
                    }
                    keep_file(dir_entry, &name, &opts, &include_matcher, &exclude_matcher, &include_re)
                });
            }
        });

    let mut id: u64 = 0;
    for entry in walker {
        let entry = entry.map_err(|e| format!("scan error: {}", e))?;
        let is_dir = entry.file_type().is_dir();
        match opts.scope {
            TargetScope::FilesOnly if is_dir => continue,
            TargetScope::DirsOnly if !is_dir => continue,
            _ => {}
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let (size, mtime_ms, ctime_ms) = stat_times(&meta);
        entries.push(FileEntry {
            id,
            path,
            is_dir,
            file_name: name,
            size,
            modified_ms: mtime_ms,
            created_ms: ctime_ms,
            selected: true,
            manual_name: None,
        });
        id += 1;
    }

    crate::core::natsort::sort_entries_by_path(&mut entries);
    log::info!(
        "scan done: {} entries in {:?} ({})",
        entries.len(),
        started.elapsed(),
        opts.path
    );
    Ok(entries)
}

fn keep_file(
    entry: &jwalk::DirEntry<((), ())>,
    name: &str,
    opts: &ScanOptions,
    include: &Option<globset::GlobSet>,
    exclude: &Option<globset::GlobSet>,
    include_re: &Option<regex::Regex>,
) -> bool {
    if name.starts_with('.') && !opts.include_hidden {
        return false;
    }
    if let Some(m) = exclude {
        if m.is_match(name) {
            return false;
        }
    }
    if let Some(m) = include {
        if !m.is_match(name) {
            return false;
        }
    }
    if let Some(re) = include_re {
        if !re.is_match(name) {
            return false;
        }
    }
    if let Ok(meta) = entry.metadata() {
        let (size, mtime_ms, _) = stat_times(&meta);
        if let Some(min) = opts.min_size {
            if size < min {
                return false;
            }
        }
        if let Some(max) = opts.max_size {
            if size > max {
                return false;
            }
        }
        if let Some(after) = opts.modified_after_ms {
            if mtime_ms < after {
                return false;
            }
        }
        if let Some(before) = opts.modified_before_ms {
            if mtime_ms > before {
                return false;
            }
        }
    }
    true
}

fn stat_times(meta: &std::fs::Metadata) -> (u64, i64, i64) {
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let ctime = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(mtime);
    (size, mtime, ctime)
}

fn build_glob_set(patterns: &[String]) -> Result<Option<globset::GlobSet>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = globset::GlobSetBuilder::new();
    for p in patterns {
        let glob = globset::GlobBuilder::new(p)
            .case_insensitive(cfg!(windows))
            .build()
            .map_err(|e| format!("invalid glob '{}': {}", p, e))?;
        builder.add(glob);
    }
    builder.build().map(Some).map_err(|e| e.to_string())
}

