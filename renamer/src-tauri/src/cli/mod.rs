pub mod presets;

use crate::core::executor;
use crate::core::metadata::MetadataCache;
use crate::core::models::*;
use crate::core::rules::{RuleParams, RuleSpec};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "renamer-cli",
    version,
    about = "Batch file renamer - preview by default, use --apply to execute",
    override_usage = "renamer-cli [OPTIONS] <COMMAND>"
)]
pub struct Cli {
    /// Operate in dry-run mode (default; kept for explicitness)
    #[arg(long, group = "mode")]
    pub dry_run: bool,

    /// Apply the renames for real
    #[arg(long, group = "mode")]
    pub apply: bool,

    /// Load a rule pipeline from a preset file (name or path)
    #[arg(long, short = 'p')]
    pub preset: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan a path and preview the planned renames
    Preview {
        /// File or folder to process
        #[arg(long, short = 'f')]
        path: String,
        #[arg(long, short = 'r')]
        recursive: bool,
        /// Glob include filters, e.g. --include "*.jpg" --include "*.png"
        #[arg(long)]
        include: Vec<String>,
    },
    /// Show saved presets
    Presets,
    /// Undo the most recent batch
    Undo,
}

pub fn run() -> i32 {
    crate::core::logger::init(true);
    log::info!("=== Batch Renamer CLI starting (v{}) ===", env!("CARGO_PKG_VERSION"));
    let cli = Cli::parse();
    let apply = cli.apply;

    match cli.command {
        Command::Presets => {
            for p in presets::list().unwrap_or_default() {
                println!("{}", p);
            }
            0
        }
        Command::Undo => {
            match crate::core::undo_store::last_batch_id() {
                Some(id) => match executor::undo_batch(&id) {
                    Ok((undone, skipped, errors)) => {
                        println!("undone={}, skipped={}", undone, skipped);
                        let has_errors = !errors.is_empty();
                        for e in errors {
                            eprintln!("error: {}", e);
                        }
                        if !has_errors { 0 } else { 3 }
                    }
                    Err(e) => {
                        eprintln!("undo failed: {}", e);
                        2
                    }
                },
                None => {
                    eprintln!("nothing to undo");
                    1
                }
            }
        }
        Command::Preview { path, recursive, include } => {
            let mut opts = ScanOptions::default();
            opts.path = path;
            opts.recursive = recursive;
            opts.include_globs = include;

            let entries = match crate::core::scanner::scan(&opts) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("scan failed: {}", e);
                    return 2;
                }
            };

            let rules: Vec<RuleSpec> = match &cli.preset {
                Some(name_or_path) => load_preset_arg(name_or_path),
                None => Vec::new(),
            };
            if let Some(r) = &cli.preset {
                if rules.is_empty() {
                    eprintln!("warning: preset '{}' loaded no rules", r);
                }
            }

            let ws = Workspace {
                entries,
                rules,
                apply_to: NamePart::Full,
                conflict_strategy: ConflictStrategy::Block,
                number_reset_per_dir: true,
            };

            let cache = MetadataCache::new();
            let report = match executor::execute_workspace(&ws, &cache, !apply, &|| false, &|done, total| {
                if apply {
                    eprint!("\r{}/{}", done, total);
                }
            }) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {}", e);
                    return 2;
                }
            };

            let preview = crate::core::preview::build_preview(&ws, &cache);
            println!("{:<8} {:<40} {:<40}", "STATUS", "OLD", "NEW");
            for item in preview.iter().take(200) {
                println!(
                    "{:<8} {:<40} {:<40}",
                    match item.status {
                        ItemStatus::Ready => "ready",
                        ItemStatus::Unchanged => "-",
                        ItemStatus::Warning => "warn",
                        ItemStatus::Error => "ERROR",
                        ItemStatus::Manual => "manual",
                    },
                    truncate(&item.old_name, 38),
                    truncate(&item.new_name, 38)
                );
            }
            if preview.len() > 200 {
                println!("... and {} more", preview.len() - 200);
            }

            if apply {
                println!("\nrenamed={}, failed={}", report.renamed, report.failed);
                if report.failed > 0 {
                    for f in &report.failures {
                        eprintln!("failed: {} -> {}: {}", f.from, f.to, f.error);
                    }
                    return 3;
                }
            } else {
                println!("\ndry-run: no files were changed. Pass --apply to rename.");
            }
            0
        }
    }
}

fn load_preset_arg(name_or_path: &str) -> Vec<RuleSpec> {
    let p = std::path::Path::new(name_or_path);
    if p.exists() {
        let content = match std::fs::read_to_string(p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("cannot read preset file: {}", e);
                return Vec::new();
            }
        };
        match serde_json::from_str::<presets::PresetFile>(&content) {
            Ok(f) => f.rules,
            Err(e) => {
                eprintln!("invalid preset file: {}", e);
                Vec::new()
            }
        }
    } else {
        presets::load(name_or_path).unwrap_or_else(|e| {
            eprintln!("preset error: {}", e);
            Vec::new()
        })
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{}…", cut)
    }
}

/// Built-in example preset used by the GUI's "sample presets" menu.
pub fn builtin_photo_preset() -> Vec<RuleSpec> {
    vec![
        RuleSpec::new(RuleParams::DateTime {
            source: "exif".into(),
            format: "%Y-%m-%d".into(),
            mode: "prefix".into(),
            separator: "_".into(),
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
