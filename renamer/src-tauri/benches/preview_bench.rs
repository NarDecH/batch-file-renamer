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

criterion_group!(benches, bench_preview);
criterion_main!(benches);
