//! Rule pipeline: registry, serialization (`RuleSpec`) and application logic.
//!
//! Every rule is a variant of [`RuleParams`]; a [`RuleSpec`] wraps it with
//! enabled/apply-to/condition metadata. Rules are applied in order; the
//! pipeline is pure (no I/O) except for lazy metadata reads through the cache.

use crate::core::metadata::MetadataCache;
use crate::core::models::NamePart;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Context passed to every rule application.
#[derive(Clone)]
pub struct RuleContext<'a> {
    /// 1-based position in the (naturally sorted) working set
    pub index: usize,
    /// Parent directory of the file
    pub parent: &'a str,
    /// Full original path
    pub path: &'a Path,
    pub size: u64,
    pub modified_ms: i64,
    pub created_ms: i64,
    /// Numbering counter within the current parent dir (1-based)
    pub index_in_dir: usize,
    /// Lazy metadata (EXIF / tags), only read when a rule asks for it
    pub metadata: &'a MetadataCache,
    /// Compiled regexes, shared for the whole preview round
    pub regex_cache: &'a crate::core::rules::regex_cache::RegexCache,
    /// Original full file name (base + ext) before any rule ran
    pub original_name: &'a str,
}

/// Parameters of each concrete rule. Serialized as a tagged enum so presets
/// round-trip cleanly and new rules can be added without breaking old files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleParams {
    Replace {
        find: String,
        replace_with: String,
        #[serde(default)]
        case_sensitive: bool,
        /// all | first | last
        #[serde(default = "default_occurrence")]
        occurrence: String,
        #[serde(default)]
        use_regex: bool,
    },
    Regex {
        pattern: String,
        /// Replacement supporting $1, ${name} capture references
        replacement: String,
    },
    Insert {
        text: String,
        /// start | end | position
        #[serde(default = "default_position")]
        at: String,
        /// For `position`: 1-based index, counted from left or right
        #[serde(default)]
        position: usize,
        #[serde(default)]
        from_right: bool,
    },
    Remove {
        /// chars | digits | symbols | spaces | brackets
        #[serde(default = "default_remove_mode")]
        mode: String,
        /// For `chars` mode: how many characters
        #[serde(default)]
        count: usize,
        #[serde(default)]
        from_right: bool,
        /// For `chars` mode: 1-based start position (from left unless from_right)
        #[serde(default)]
        start: usize,
        /// For `chars` mode: 1-based end position (inclusive); 0 = to the end
        #[serde(default)]
        end: usize,
    },
    Case {
        /// upper | lower | title | sentence | camel | snake | kebab
        style: String,
    },
    Numbering {
        #[serde(default = "default_one")]
        start: usize,
        #[serde(default = "default_one")]
        step: usize,
        #[serde(default = "default_pad")]
        pad: usize,
        /// prefix | suffix | replace
        #[serde(default = "default_numbering_mode")]
        mode: String,
        /// Text joined with the number, e.g. "_" for "_001"
        #[serde(default)]
        separator: String,
        #[serde(default)]
        reset_per_dir: bool,
    },
    Extension {
        /// set | remove | lowercase
        action: String,
        /// New extension without dot (for `set`)
        #[serde(default)]
        extension: String,
    },
    DateTime {
        /// created | modified | exif
        source: String,
        /// chrono strftime format, e.g. "%Y-%m-%d"
        format: String,
        /// prefix | suffix | replace
        #[serde(default = "default_numbering_mode")]
        mode: String,
        #[serde(default)]
        separator: String,
    },
    Metadata {
        /// exif_camera | exif_size | artist | title | album | track
        field: String,
        /// prefix | suffix | replace
        #[serde(default = "default_numbering_mode")]
        mode: String,
        #[serde(default)]
        separator: String,
        /// Fallback when the metadata value is missing
        #[serde(default)]
        fallback: String,
    },
    Template {
        /// e.g. "{name}_{n:03}_{modified:%Y%m%d}.{ext}"
        template: String,
    },
    Cleanup {
        #[serde(default = "default_true")]
        trim: bool,
        #[serde(default = "default_true")]
        collapse_spaces: bool,
        /// Replace spaces with this string ("" = keep)
        #[serde(default)]
        space_replacement: String,
        #[serde(default = "default_true")]
        strip_forbidden: bool,
        /// Transliterate accents: é -> e
        #[serde(default)]
        strip_accents: bool,
        /// Unicode NFC normalization
        #[serde(default = "default_true")]
        normalize_nfc: bool,
    },
    Swap {
        delimiter: String,
    },
    ImportList {
        /// Mapping old name -> new name (explicit mapping mode)
        #[serde(default)]
        mapping: Vec<(String, String)>,
        /// Ordered list of new names applied by working-set position
        #[serde(default)]
        ordered_names: Vec<String>,
    },
    Hash {
        /// md5 | sha1 | uuid
        algo: String,
        /// Full hash or truncated to N chars (0 = full)
        #[serde(default)]
        length: usize,
        /// Keep the original extension
        #[serde(default = "default_true")]
        keep_extension: bool,
    },
}

fn default_occurrence() -> String { "all".into() }
fn default_position() -> String { "end".into() }
fn default_remove_mode() -> String { "chars".into() }
fn default_one() -> usize { 1 }
fn default_pad() -> usize { 3 }
fn default_numbering_mode() -> String { "suffix".into() }
fn default_true() -> bool { true }

/// A rule in the pipeline with its metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleSpec {
    pub params: RuleParams,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Override of the workspace-level apply_to: name | extension | full
    #[serde(default)]
    pub apply_to: Option<NamePart>,
    /// Only apply when the original name matches this glob, e.g. "*.jpg"
    #[serde(default)]
    pub only_if_glob: Option<String>,
}

impl RuleSpec {
    pub fn new(params: RuleParams) -> Self {
        RuleSpec { params, enabled: true, apply_to: None, only_if_glob: None }
    }
}

/// Result of applying the whole pipeline to one file.
pub struct PipelineOutput {
    /// New base name (without extension)
    pub name: String,
    /// New extension (without dot)
    pub extension: String,
}

/// Apply the pipeline `rules` to `original_name` (base name only, no dot+ext
/// handling here - the caller splits/parts appropriately).
pub fn apply_pipeline(
    rules: &[RuleSpec],
    workspace_apply_to: NamePart,
    base: &mut String,
    extension: &mut String,
    ctx: &RuleContext,
) {
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if let Some(glob) = &rule.only_if_glob {
            if !glob_match(glob, ctx.original_name) {
                continue;
            }
        }
        let target = rule.apply_to.unwrap_or(workspace_apply_to);
        // Build the string the rule operates on
        let mut work = match target {
            NamePart::Name => base.clone(),
            NamePart::Extension => extension.clone(),
            NamePart::Full => {
                if extension.is_empty() { base.clone() } else { format!("{}.{}", base, extension) }
            }
        };
        work = match target {
            NamePart::Full => apply_full_aware(&rule.params, &work, ctx),
            _ => crate::core::rules::impls::apply_rule(&rule.params, &work, ctx),
        };
        match target {
            NamePart::Name => *base = work,
            NamePart::Extension => *extension = work,
            NamePart::Full => {
                // Re-split full name: extension = after last dot, only if the
                // rule did not remove it. If there is no dot, everything is base.
                match work.rfind('.') {
                    Some(i) if i > 0 && i + 1 < work.len() => {
                        *base = work[..i].to_string();
                        *extension = work[i + 1..].to_string();
                    }
                    _ => {
                        *base = work;
                        *extension = String::new();
                    }
                }
            }
        }
    }
}

/// In `Full` mode some rules must not touch the extension blindly:
/// - Numbering/DateTime/Metadata `suffix` insert before the extension
/// - Extension rules act on the extension part only
fn apply_full_aware(params: &RuleParams, work: &str, ctx: &RuleContext) -> String {
    let is_ext_rule = matches!(params, RuleParams::Extension { .. });
    let base_only = match params {
        RuleParams::Numbering { .. } => true,
        RuleParams::DateTime { mode, .. } | RuleParams::Metadata { mode, .. } if mode == "suffix" => true,
        _ => false,
    };
    if is_ext_rule || base_only {
        let (base, ext) = split_last_dot(work);
        let target_str = if is_ext_rule { &ext } else { &base };
        let new_part = crate::core::rules::impls::apply_rule(params, target_str, ctx);
        if is_ext_rule {
            if new_part.is_empty() {
                return base;
            }
            if base.is_empty() && ext.is_empty() {
                return new_part;
            }
            return format!("{}.{}", base, new_part);
        }
        // base-only rules
        if ext.is_empty() {
            return new_part;
        }
        return format!("{}.{}", new_part, ext);
    }
    crate::core::rules::impls::apply_rule(params, work, ctx)
}

pub(crate) fn split_last_dot(s: &str) -> (String, String) {
    match s.rfind('.') {
        Some(i) if i > 0 && i + 1 < s.len() => (s[..i].to_string(), s[i + 1..].to_string()),
        _ => (s.to_string(), String::new()),
    }
}

/// Minimal glob matcher supporting * ? and literal characters.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn rec(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some('?'), Some(_)) => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b) && rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    rec(&p, &t)
}

/// Validate a rule's parameters (e.g. regex compiles). Returns an error message.
pub fn validate_rule(rule: &RuleSpec) -> Result<(), String> {
    match &rule.params {
        RuleParams::Replace { use_regex: true, find, .. } => {
            regex::Regex::new(find).map(|_| ()).map_err(|e| format!("bad regex '{}': {}", find, e))
        }
        RuleParams::Regex { pattern, .. } => {
            regex::Regex::new(pattern).map(|_| ()).map_err(|e| format!("bad regex '{}': {}", pattern, e))
        }
        _ => Ok(()),
    }
}
