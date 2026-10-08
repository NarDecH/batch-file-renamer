//! Acceptance tests (spec criteria 1-7). Each test maps to one criterion.

use renamer_lib::core::executor;
use renamer_lib::core::metadata::MetadataCache;
use renamer_lib::core::models::*;
use renamer_lib::core::preview::{build_preview, split_name_ext};
use renamer_lib::core::rules::{RuleParams, RuleSpec};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn rule(p: RuleParams) -> RuleSpec {
    RuleSpec::new(p)
}

fn workspace(entries: Vec<FileEntry>, rules: Vec<RuleSpec>) -> Workspace {
    Workspace {
        entries,
        rules,
        apply_to: NamePart::Full,
        conflict_strategy: ConflictStrategy::Block,
        number_reset_per_dir: true,
    }
}

fn make_entry(id: u64, dir: &std::path::Path, name: &str) -> FileEntry {
    FileEntry {
        id,
        path: dir.join(name),
        is_dir: false,
        file_name: name.to_string(),
        size: 10,
        modified_ms: 1_710_000_000_000,
        created_ms: 1_710_000_000_000,
        selected: true,
        manual_name: None,
    }
}

fn preview_names(ws: &Workspace, cache: &MetadataCache) -> Vec<(String, String, ItemStatus)> {
    build_preview(ws, cache)
        .into_iter()
        .map(|i| (i.old_name, i.new_name, i.status))
        .collect()
}

/// Criterion 1: IMG_20240315_143022.JPG -> Trip_2024-03-15_001.jpg
#[test]
fn criterion_1_full_pipeline() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let entry = make_entry(0, dir, "IMG_20240315_143022.JPG");
    let ws = workspace(
        vec![entry],
        vec![
            rule(RuleParams::Regex {
                pattern: "IMG_(\\d{4})(\\d{2})(\\d{2})_\\d+".into(),
                replacement: "$1-$2-$3".into(),
            }),
            rule(RuleParams::Insert {
                text: "Trip_".into(),
                at: "start".into(),
                position: 0,
                from_right: false,
            }),
            rule(RuleParams::Numbering {
                start: 1,
                step: 1,
                pad: 3,
                mode: "suffix".into(),
                separator: "_".into(),
                reset_per_dir: true,
            }),
            rule(RuleParams::Extension {
                action: "lowercase".into(),
                extension: String::new(),
            }),
        ],
    );
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    assert_eq!(preview.len(), 1);
    let (_, new, status) = &preview[0];
    assert_eq!(new, "Trip_2024-03-15_001.jpg", "criterion 1 failed");
    assert_eq!(*status, ItemStatus::Ready);
}

/// Criterion 2: swap a.txt <-> b.txt in one batch, contents intact.
#[test]
fn criterion_2_swap_two_files() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("a.txt"), "content-a").unwrap();
    fs::write(dir.join("b.txt"), "content-b").unwrap();

    let entries = vec![make_entry(0, dir, "a.txt"), make_entry(1, dir, "b.txt")];
    let mut a = entries[0].clone();
    a.manual_name = Some("b.txt".into());
    let mut b = entries[1].clone();
    b.manual_name = Some("a.txt".into());
    let ws = workspace(vec![a, b], vec![]);

    let cache = MetadataCache::new();
    let report = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert!(report.failures.is_empty(), "unexpected failures: {:?}", report.failures);

    assert_eq!(report.renamed, 2, "both renames must succeed (cycle temp handling)");
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "content-b");
    assert_eq!(fs::read_to_string(dir.join("b.txt")).unwrap(), "content-a");
}

/// Criterion 3: photo.JPG -> photo.jpg is allowed, not a duplicate.
#[test]
fn criterion_3_case_only_rename() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("photo.JPG"), "x").unwrap();
    let entry = make_entry(0, dir, "photo.JPG");
    let mut e = entry.clone();
    e.manual_name = Some("photo.jpg".into());
    let ws = workspace(vec![e], vec![]);

    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    // Manual override that passes validation must be runnable (Ready/Manual)
    assert!(matches!(preview[0].2, ItemStatus::Manual | ItemStatus::Ready), "got {:?}", preview[0].2);

    let report = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, 1);
    assert!(dir.join("photo.jpg").exists());
}

/// Criterion 4: two files becoming report.txt warn, auto-suffix gives
/// report.txt and report (1).txt.
#[test]
fn criterion_4_duplicate_auto_suffix() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let mut a = make_entry(0, dir, "a.txt");
    a.manual_name = Some("report.txt".into());
    let mut b = make_entry(1, dir, "b.txt");
    b.manual_name = Some("report.txt".into());
    let ws = workspace(vec![a, b], vec![]);

    let cache = MetadataCache::new();
    let preview = build_preview(&ws, &cache);

    // At least one conflict must be detected
    let conflicts = preview.iter().filter(|i| matches!(i.status, ItemStatus::Error | ItemStatus::Warning)).count();
    assert!(conflicts >= 1, "duplicates must be flagged");

    // Now with AutoSuffix
    let ws2 = Workspace { conflict_strategy: ConflictStrategy::AutoSuffix, ..ws };
    let preview2 = build_preview(&ws2, &cache);
    let mut names: Vec<String> = preview2.iter().map(|i| i.new_name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["report (1).txt", "report.txt"]);
}

/// Criterion 5: natural sort + numbering -> 001, 002, 003 for ep1, ep2, ep10.
#[test]
fn criterion_5_natural_sort_numbering() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let entries: Vec<FileEntry> = vec!["ep1.mp4", "ep2.mp4", "ep10.mp4"]
        .iter()
        .enumerate()
        .map(|(i, n)| make_entry(i as u64, dir, n))
        .collect();
    // Scanner sorts naturally; emulate by sorting here
    let mut sorted = entries;
    sorted.sort_by(|a, b| renamer_lib::core::natsort::natural_cmp(&a.file_name, &b.file_name));
    for (i, e) in sorted.iter_mut().enumerate() {
        e.id = i as u64;
    }
    let ws = workspace(
        sorted,
        vec![rule(RuleParams::Numbering {
            start: 1,
            step: 1,
            pad: 3,
            mode: "replace".into(),
            separator: String::new(),
            reset_per_dir: true,
        })],
    );
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    let nums: Vec<&str> = preview.iter().map(|(_, new, _)| new.as_str()).collect();
    assert_eq!(nums, vec!["001.mp4", "002.mp4", "003.mp4"]);
}

/// Criterion 6: undo works across program restarts (journal is on disk).
#[test]
fn criterion_6_undo_after_restart() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("one.txt"), "1").unwrap();
    fs::write(dir.join("two.txt"), "2").unwrap();

    let entries = vec![make_entry(0, dir, "one.txt"), make_entry(1, dir, "two.txt")];
    let mut a = entries[0].clone();
    a.manual_name = Some("first.txt".into());
    let mut b = entries[1].clone();
    b.manual_name = Some("second.txt".into());
    let ws = workspace(vec![a, b], vec![]);

    let cache = MetadataCache::new();
    let report = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, 2);

    // Simulate restart: undo via journal file (no in-memory state)
    let (undone, _skipped, errors) = executor::undo_batch(&report.batch_id).unwrap();
    assert_eq!(undone, 2, "undo must restore every file; errors: {:?}", errors);
    assert!(dir.join("one.txt").exists() && dir.join("two.txt").exists());
    assert!(!dir.join("first.txt").exists() && !dir.join("second.txt").exists());
    assert_eq!(fs::read_to_string(dir.join("one.txt")).unwrap(), "1");
}

/// Criterion 7: Thai + emoji names survive all rules; removing the first
/// grapheme never leaves a floating vowel/tone mark.
#[test]
fn criterion_7_thai_emoji_grapheme_safety() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let name = "น้ำตก ภูกระดึง 😀.jpg";
    let entry = make_entry(0, dir, name);

    // Remove first character (grapheme-safe)
    let remove_rule = rule(RuleParams::Remove {
        mode: "chars".into(),
        count: 1,
        from_right: false,
        start: 0,
        end: 0,
    });
    let ws = workspace(vec![entry.clone()], vec![remove_rule]);
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    let new_name = &preview[0].1;

    // Grapheme segmentation keeps "น้ำ" together (น + ้ + ำ is one cluster),
    // so removing 1 grapheme removes the whole syllable-initial unit.
    assert_eq!(new_name, "ตก ภูกระดึง 😀.jpg");
    // The critical assertion: the result starts with a valid base character,
    // not a combining mark.
    let first = new_name.chars().next().unwrap();
    assert!(
        !is_thai_combining(first),
        "first char must not be a floating Thai vowel/tone mark: {}",
        new_name
    );

    // Full pipeline (cleanup + case) preserves the name
    let ws2 = workspace(
        vec![entry],
        vec![
            rule(RuleParams::Cleanup {
                trim: true,
                collapse_spaces: true,
                space_replacement: "_".into(),
                strip_forbidden: true,
                strip_accents: false,
                normalize_nfc: true,
            }),
        ],
    );
    let preview2 = preview_names(&ws2, &cache);
    assert_eq!(preview2[0].1, "น้ำตก_ภูกระดึง_😀.jpg");
}

fn is_thai_combining(c: char) -> bool {
    matches!(c as u32, 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E)
}

/// Extra: reserved Windows names and forbidden characters are rejected.
#[test]
fn validation_reserved_names() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let mut e = make_entry(0, dir, "ok.txt");
    e.manual_name = Some("con.txt".into());
    let ws = workspace(vec![e], vec![]);
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    assert_eq!(preview[0].2, ItemStatus::Error);

    let mut e2 = make_entry(1, dir, "ok2.txt");
    e2.manual_name = Some("bad|name.txt".into());
    let ws2 = workspace(vec![e2], vec![]);
    let preview2 = preview_names(&ws2, &cache);
    assert_eq!(preview2[0].2, ItemStatus::Error);
}

/// Extra: chain rename A->B, B->C works without temp files left behind.
#[test]
fn chain_rename_no_leftovers() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("A"), "a").unwrap();
    fs::write(dir.join("B"), "b").unwrap();

    let mut a = make_entry(0, dir, "A");
    a.manual_name = Some("B".into());
    let mut b = make_entry(1, dir, "B");
    b.manual_name = Some("C".into());
    let ws = workspace(vec![a, b], vec![]);

    let cache = MetadataCache::new();
    let report = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, 2);
    assert!(dir.join("B").exists() && dir.join("C").exists() && !dir.join("A").exists());
    assert_eq!(fs::read_to_string(dir.join("B")).unwrap(), "a");
    assert_eq!(fs::read_to_string(dir.join("C")).unwrap(), "b");
    // No temp leftovers
    let leftovers: Vec<PathBuf> = fs::read_dir(dir).unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap_or_default().to_string_lossy().contains("__rntmp__"))
        .collect();
    assert!(leftovers.is_empty(), "temp files leaked: {:?}", leftovers);
}

/// Extra: dry-run touches nothing.
#[test]
fn dry_run_changes_nothing() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("a.txt"), "x").unwrap();
    let mut e = make_entry(0, dir, "a.txt");
    e.manual_name = Some("b.txt".into());
    let ws = workspace(vec![e], vec![]);
    let cache = MetadataCache::new();
    let report = executor::execute_workspace(&ws, &cache, true, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, 1);
    assert!(dir.join("a.txt").exists());
    assert!(!dir.join("b.txt").exists());
}

/// Extra: split_name_ext basic behavior.
#[test]
fn split_name_ext_basics() {
    assert_eq!(split_name_ext("photo.JPG"), ("photo".into(), "JPG".into()));
    assert_eq!(split_name_ext("archive.tar.gz"), ("archive.tar".into(), "gz".into()));
    assert_eq!(split_name_ext(".gitignore"), (".gitignore".into(), String::new()));
}
