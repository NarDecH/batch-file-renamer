//! Benchmark: preview of 10,000 files with a 5-rule pipeline.
//! Acceptance criterion: ~200 ms or less (excluding metadata reads).

use criterion::{criterion_group, criterion_main, Criterion};
use renamer_lib::core::metadata::MetadataCache;
use renamer_lib::core::models::*;
use renamer_lib::core::preview::build_preview;
use renamer_lib::core::rules::{RuleParams, RuleSpec};

fn five_rules() -> Vec<RuleSpec> {
    vec![
        RuleSpec::new(RuleParams::Replace {
            find: "_old".into(),
            replace_with: "_new".into(),
            case_sensitive: false,
            occurrence: "all".into(),
            use_regex: false,
        }),
        RuleSpec::new(RuleParams::Regex {
            pattern: r"IMG_(\d{4})(\d{2})(\d{2})".into(),
            replacement: "$1-$2-$3".into(),
        }),
        RuleSpec::new(RuleParams::Insert {
            text: "Trip_".into(),
            at: "start".into(),
            position: 0,
            from_right: false,
        }),
        RuleSpec::new(RuleParams::Numbering {
            start: 1,
            step: 1,
            pad: 3,
            mode: "suffix".into(),
            separator: "_".into(),
            reset_per_dir: true,
        }),
        RuleSpec::new(RuleParams::Cleanup {
            trim: true,
            collapse_spaces: true,
            space_replacement: "_".into(),
            strip_forbidden: true,
            strip_accents: false,
            normalize_nfc: true,
        }),
    ]
}

fn make_workspace(n: usize) -> Workspace {
    let mut entries = Vec::with_capacity(n);
    for i in 0..n {
        let name = format!("IMG_20240315_{:06}_old.JPG", i);
        entries.push(FileEntry {
            id: i as u64,
            path: std::path::PathBuf::from(format!("C:\\tmp\\bench\\{}", name)),
            is_dir: false,
            file_name: name,
            size: 1024,
            modified_ms: 1_710_000_000_000,
            created_ms: 1_710_000_000_000,
            selected: true,
            manual_name: None,
        });
    }
    Workspace {
        entries,
        rules: five_rules(),
        apply_to: NamePart::Full,
        conflict_strategy: ConflictStrategy::Block,
        number_reset_per_dir: true,
    }
}

fn bench_preview(c: &mut Criterion) {
    let ws = make_workspace(10_000);
    let cache = MetadataCache::new();
    c.bench_function("preview_10k_5rules", |b| b.iter(|| build_preview(&ws, &cache)));
}

use renamer_lib::core::executor;

/// Benchmark: execute 10,000 real file renames on disk with one batch.
/// Measures the full rename+journal path after the O(n²)→O(1) executor fixes.
/// Criterion's repeated iterations would hit "destination already exists", so
/// we run exactly one timed pass before/after style: rename everything, then
/// restore names, then rename again - timing only the first batch.
fn bench_execute_10k(c: &mut Criterion) {
    let dir = std::env::temp_dir().join(format!("rn_bench_{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let n = 10_000usize;
    let mut entries = Vec::with_capacity(n);
    for i in 0..n {
        let name = format!("sample_{:06}.txt", i);
        let p = dir.join(&name);
        std::fs::write(&p, b"x").unwrap();
        entries.push(FileEntry {
            id: i as u64,
            path: p,
            is_dir: false,
            file_name: name,
            size: 1,
            modified_ms: 0,
            created_ms: 0,
            selected: true,
            manual_name: Some(format!("renamed_{:06}.txt", i)),
        });
    }
    let ws = Workspace {
        entries,
        rules: vec![],
        apply_to: NamePart::Full,
        conflict_strategy: ConflictStrategy::Block,
        number_reset_per_dir: true,
    };
    let cache = MetadataCache::new();

    // Manual timing: single forward batch is the number we care about.
    let started = std::time::Instant::now();
    let report =
        executor::execute_workspace(&ws, &cache, false, &(|| false), &|_, _| {}).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(report.renamed, 10_000, "bench setup must rename all files");
    println!(
        "execute_10k_manual_files: 10,000 renames in {:?} (batch_id={})",
        elapsed, report.batch_id
    );

    let dir_for_bench = dir.clone();
    c.bench_function("execute_10k_manual_files", move |b| {
        // Each extra iteration: restore then re-rename, still useful to average
        // out disk noise across the restore step which we exclude by timing
        // only execute_workspace inside.
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                // Restore original names (untimed) - reverse all renames
                for e in dir_for_bench.read_dir().unwrap().flatten() {
                    let p = e.path();
                    if let Some(fname) = p.file_name().and_then(|f| f.to_str()) {
                        if let Some(num) = fname.strip_prefix("renamed_") {
                            let _ = std::fs::rename(&p, dir_for_bench.join(format!("sample_{}", num)));
                        }
                    }
                }
                let t0 = std::time::Instant::now();
                let r =
                    executor::execute_workspace(&ws, &cache, false, &(|| false), &|_, _| {})
                        .unwrap();
                total += t0.elapsed();
                assert_eq!(r.renamed, 10_000);
            }
            total
        })
    });
    let _ = std::fs::remove_dir_all(&dir);
}

criterion_group!(benches, bench_preview, bench_execute_10k);
criterion_main!(benches);
