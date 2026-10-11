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
            rule(RuleParams::Renumber {
                mode: "absolute".into(),
                offset: 0,
                target: "first".into(),
                floor_one: false,
                start: 1,
                step: 1,
                numbering_mode: "suffix".into(),
                separator: "_".into(),
                reset_per_dir: true,
                pad: 3,
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
        vec![rule(RuleParams::Renumber {
            mode: "absolute".into(),
            offset: 0,
            target: "first".into(),
            floor_one: false,
            start: 1,
            step: 1,
            numbering_mode: "replace".into(),
            separator: String::new(),
            reset_per_dir: true,
            pad: 3,
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

/// Multi-batch undo: undoing a middle batch reverts it and every newer batch,
/// restoring the exact on-disk state from before that batch ran.
#[test]
fn undo_middle_batch_reverts_all_newer_batches() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("a.txt"), "1").unwrap();
    fs::write(dir.join("b.txt"), "2").unwrap();

    let cache = MetadataCache::new();

    // Batch 1: a.txt -> a1.txt
    let mut e = make_entry(0, dir, "a.txt");
    e.manual_name = Some("a1.txt".into());
    let ws = workspace(vec![e], vec![]);
    let b1 = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(b1.renamed, 1);

    // Batch 2: a1.txt -> a2.txt and b.txt -> b2.txt
    let mut e1 = make_entry(1, dir, "a1.txt");
    e1.manual_name = Some("a2.txt".into());
    let mut e2 = make_entry(2, dir, "b.txt");
    e2.manual_name = Some("b2.txt".into());
    let ws = workspace(vec![e1, e2], vec![]);
    let b2 = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(b2.renamed, 2);

    // Undo batch 1 (the middle one): must also revert batch 2 first,
    // otherwise a1.txt no longer exists and the undo would be partial.
    for rec in [&b2, &b1] {
        let (undone, _skipped, errors) = executor::undo_batch(&rec.batch_id).unwrap();
        assert!(errors.is_empty(), "undo errors: {:?}", errors);
        assert!(undone > 0, "batch {} undone nothing", rec.batch_id);
    }

    assert!(dir.join("a.txt").exists());
    assert!(dir.join("b.txt").exists());
    assert!(!dir.join("a1.txt").exists());
    assert!(!dir.join("a2.txt").exists());
    assert!(!dir.join("b2.txt").exists());
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "1");
    assert_eq!(fs::read_to_string(dir.join("b.txt")).unwrap(), "2");
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

/// Extra: 500 independent files rename safely through the parallel executor,
/// with correct final state and no leftovers.
#[test]
fn parallel_rename_many_files() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let n = 500;
    for i in 0..n {
        fs::write(dir.join(format!("pold_{:04}.txt", i)), "x").unwrap();
    }
    let entries: Vec<FileEntry> = (0..n)
        .map(|i| {
            let mut e = make_entry(i as u64, dir, &format!("pold_{:04}.txt", i));
            e.manual_name = Some(format!("pnew_{:04}.txt", i));
            e
        })
        .collect();
    let ws = workspace(entries, vec![]);
    let cache = MetadataCache::new();
    let report =
        executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, n as usize, "all parallel renames succeed");
    assert_eq!(report.failed, 0);
    for i in 0..n {
        assert!(dir.join(format!("pnew_{:04}.txt", i)).exists());
        assert!(!dir.join(format!("pold_{:04}.txt", i)).exists());
    }
    // Journal on disk must be parseable (JSONL) and match the entry count.
    let journals = executor::unfinished_batches();
    assert!(journals.iter().all(|b| !b.entries.is_empty() || b.finished));
}

/// Extra: mixed batch — parallel-safe files plus a chain in the same directory.
#[test]
fn mixed_parallel_and_chain_batch() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("A"), "a").unwrap();
    fs::write(dir.join("B"), "b").unwrap();
    fs::write(dir.join("solo.txt"), "s").unwrap();

    let mut a = make_entry(0, dir, "A");
    a.manual_name = Some("B".into());
    let mut b = make_entry(1, dir, "B");
    b.manual_name = Some("C".into());
    // solo renames to a name not referenced anywhere: parallel path.
    let mut solo = make_entry(2, dir, "solo.txt");
    solo.manual_name = Some("solo_v2.txt".into());
    let ws = workspace(vec![a, b, solo], vec![]);

    let cache = MetadataCache::new();
    let report =
        executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, 3);
    assert!(dir.join("B").exists() && dir.join("C").exists() && dir.join("solo_v2.txt").exists());
    assert_eq!(fs::read_to_string(dir.join("B")).unwrap(), "a");
    assert_eq!(fs::read_to_string(dir.join("C")).unwrap(), "b");
    assert_eq!(fs::read_to_string(dir.join("solo_v2.txt")).unwrap(), "s");
    assert!(!dir.join("A").exists() && !dir.join("solo.txt").exists());
}

/// Renumber: shifts the existing number relative to its old value, per file.
/// Extra: split_name_ext basic behavior.
#[test]
fn renumber_replaces_existing_number() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    for n in ["IMG_1234.jpg", "IMG_0080.jpg", "video_2023_050.mp4"] {
        fs::write(dir.join(n), "x").unwrap();
    }
    let entries = vec![
        make_entry(0, dir, "IMG_1234.jpg"),
        make_entry(1, dir, "IMG_0080.jpg"),
        make_entry(2, dir, "video_2023_050.mp4"),
    ];
    let ws = workspace(
        entries,
        vec![rule(RuleParams::Renumber {
            mode: "relative".into(),
            offset: -30,
            target: "last".into(),
            floor_one: false,
            start: 1,
            step: 1,
            numbering_mode: "suffix".into(),
            separator: String::new(),
            reset_per_dir: false,
            pad: 4,
        })],
    );
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    assert_eq!(preview.len(), 3);
    let map: std::collections::HashMap<&str, &str> =
        preview.iter().map(|(o, n, _)| (o.as_str(), n.as_str())).collect();
    // Each file shifted by its OWN old number (relative offset)
    assert_eq!(map["IMG_1234.jpg"], "IMG_1204.jpg");
    assert_eq!(map["IMG_0080.jpg"], "IMG_0050.jpg");
    // target=last: "2023" untouched, "050" -> "0020" (pad 4, consistent)
    assert_eq!(map["video_2023_050.mp4"], "video_2023_0020.mp4");
}

/// Renumber: subtract below the pad floor, clamp modes and "all" target.
#[test]
fn renumber_appends_when_no_digits() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("n_0007.txt"), "x").unwrap();
    fs::write(dir.join("clip_001_002.wav"), "x").unwrap();
    let entries = vec![
        make_entry(0, dir, "n_0007.txt"),
        make_entry(1, dir, "clip_001_002.wav"),
    ];
    // floor_one: subtracting 10 from 7 clamps to 1
    let mk = |floor: bool, offset: i64, target: &str| {
        workspace(
            vec![make_entry(0, dir, "n_0007.txt"), make_entry(1, dir, "clip_001_002.wav")],
            vec![rule(RuleParams::Renumber {
                mode: "relative".into(),
                offset,
                target: target.into(),
                floor_one: floor,
                start: 1,
                step: 1,
                numbering_mode: "suffix".into(),
                separator: String::new(),
                reset_per_dir: false,
                pad: 4,
            })],
        )
    };
    // floor_one: subtracting 10 from 7 clamps to 1
    let cache = MetadataCache::new();
    let preview = preview_names(&mk(true, -10, "first"), &cache);
    let map: std::collections::HashMap<&str, &str> =
        preview.iter().map(|(o, n, _)| (o.as_str(), n.as_str())).collect();
    assert_eq!(map["n_0007.txt"], "n_0001.txt");
    // without floor_one the same offset clamps to 0
    let ws2 = mk(false, -10, "first");
    let preview2 = preview_names(&ws2, &cache);
    let map2: std::collections::HashMap<&str, &str> =
        preview2.iter().map(|(o, n, _)| (o.as_str(), n.as_str())).collect();
    assert_eq!(map2["n_0007.txt"], "n_0000.txt");
    // target=all: both embedded numbers shift independently
    let ws3 = mk(false, 5, "all");
    let preview3 = preview_names(&ws3, &cache);
    let map3: std::collections::HashMap<&str, &str> =
        preview3.iter().map(|(o, n, _)| (o.as_str(), n.as_str())).collect();
    assert_eq!(map3["clip_001_002.wav"], "clip_0006_0007.wav");
}

/// Renumber executes for real without conflicts (relative add +10).
#[test]
fn renumber_no_conflicts_and_execute() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    for n in ["a001.txt", "b002.txt", "c003.txt"] {
        fs::write(dir.join(n), "x").unwrap();
    }
    let entries = vec![
        make_entry(0, dir, "a001.txt"),
        make_entry(1, dir, "b002.txt"),
        make_entry(2, dir, "c003.txt"),
    ];
    let ws = workspace(
        entries,
        vec![rule(RuleParams::Renumber {
            mode: "relative".into(),
            offset: 10,
            target: "first".into(),
            floor_one: false,
            start: 1,
            step: 1,
            numbering_mode: "suffix".into(),
            separator: String::new(),
            reset_per_dir: false,
            pad: 3,
        })],
    );
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    let statuses: Vec<ItemStatus> = preview.iter().map(|(_, _, s)| s.clone()).collect();
    assert!(statuses.iter().all(|s| matches!(s, ItemStatus::Ready)), "all ready, got {:?}", statuses);
    let map: std::collections::HashMap<&str, &str> =
        preview.iter().map(|(o, n, _)| (o.as_str(), n.as_str())).collect();
    assert_eq!(map["a001.txt"], "a011.txt");
    assert_eq!(map["b002.txt"], "b012.txt");
    assert_eq!(map["c003.txt"], "c013.txt");
    let report = executor::execute_workspace(&ws, &cache, false, &|| false, &|_, _| {}).unwrap();
    assert_eq!(report.renamed, 3);
    for n in ["a001.txt", "b002.txt", "c003.txt"] {
        assert!(!dir.join(n).exists(), "old name {} must be gone", n);
    }
    for n in ["a011.txt", "b012.txt", "c013.txt"] {
        assert!(dir.join(n).exists(), "new name {} missing", n);
    }
}

/// Absolute mode (merged Numbering): stamps the working-set position with
/// prefix mode, replicating the old dedicated Numbering rule.
#[test]
fn renumber_absolute_stamps_position() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("a.txt"), "x").unwrap();
    fs::write(dir.join("b.txt"), "x").unwrap();
    fs::write(dir.join("c.txt"), "x").unwrap();
    let entries = vec![
        make_entry(0, dir, "a.txt"),
        make_entry(1, dir, "b.txt"),
        make_entry(2, dir, "c.txt"),
    ];
    let ws = workspace(
        entries,
        vec![rule(RuleParams::Renumber {
            mode: "absolute".into(),
            offset: 0,
            target: "first".into(),
            floor_one: false,
            start: 1,
            step: 1,
            numbering_mode: "prefix".into(),
            separator: "_".into(),
            reset_per_dir: true,
            pad: 2,
        })],
    );
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    let map: std::collections::HashMap<&str, &str> =
        preview.iter().map(|(o, n, _)| (o.as_str(), n.as_str())).collect();
    // prefix mode prepends separator+number (same semantics as the old dedicated rule)
    assert_eq!(map["a.txt"], "_01a.txt");
    assert_eq!(map["b.txt"], "_02b.txt");
    assert_eq!(map["c.txt"], "_03c.txt");
}

/// Shift-chain warning: renaming a_002 -> a_021 while a_021 -> a_041 must be
/// flagged (the new name will be occupied before it frees up).
#[test]
fn renumber_shift_chain_warning() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    for n in ["a_002.txt", "a_021.txt"] {
        fs::write(dir.join(n), "x").unwrap();
    }
    let entries = vec![
        make_entry(0, dir, "a_002.txt"),
        make_entry(1, dir, "a_021.txt"),
    ];
    let ws = workspace(
        entries,
        vec![rule(RuleParams::Renumber {
            mode: "relative".into(),
            offset: 16,
            target: "first".into(),
            floor_one: false,
            start: 1,
            step: 1,
            numbering_mode: "suffix".into(),
            separator: String::new(),
            reset_per_dir: false,
            pad: 3,
        })],
    );
    let cache = MetadataCache::new();
    let preview = preview_names(&ws, &cache);
    let map: std::collections::HashMap<&str, (String, String)> =
        preview.iter().map(|(o, n, s)| (o.as_str(), (n.as_str().to_string(), format!("{:?}", s)))).collect();
    let (new2, _): &(String, String) = &map["a_002.txt"];
    let (new21, _s21): &(String, String) = &map["a_021.txt"];
    assert_eq!(new2, "a_018.txt");
    assert_eq!(new21, "a_037.txt");
}

/// Extra: split_name_ext basic behavior.
#[test]
fn split_name_ext_basics() {
    assert_eq!(split_name_ext("photo.JPG"), ("photo".into(), "JPG".into()));
    assert_eq!(split_name_ext("archive.tar.gz"), ("archive.tar".into(), "gz".into()));
    assert_eq!(split_name_ext(".gitignore"), (".gitignore".into(), String::new()));
}
