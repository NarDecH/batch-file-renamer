//! Concrete rule implementations. All string surgery is grapheme-aware so
//! Thai vowels/tone marks and emoji never get separated from their base.

use super::pipeline::{glob_match, RuleContext, RuleParams};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Apply one rule to the working string.
pub fn apply_rule(params: &RuleParams, input: &str, ctx: &RuleContext) -> String {
    match params {
        RuleParams::Replace { find, replace_with, case_sensitive, occurrence, use_regex } => {
            apply_replace(input, find, replace_with, *case_sensitive, occurrence, *use_regex, ctx)
        }
        RuleParams::Regex { pattern, replacement } => match ctx.regex_cache.get(pattern) {
            Some(re) => re.replace_all(input, replacement.as_str()).into_owned(),
            None => input.to_string(),
        },
        RuleParams::Insert { text, at, position, from_right } => {
            apply_insert(input, text, at, *position, *from_right)
        }
        RuleParams::Remove { mode, count, from_right, start, end } => {
            apply_remove(input, mode, *count, *from_right, *start, *end)
        }
        RuleParams::Case { style } => apply_case(input, style),
        RuleParams::Numbering { start, step, pad, mode, separator, reset_per_dir } => {
            let idx = if *reset_per_dir { ctx.index_in_dir } else { ctx.index };
            let number = start + (idx.saturating_sub(1)) * step;
            let num_str = format!("{:0width$}", number, width = pad);
            let piece = format!("{}{}", separator, num_str);
            match mode.as_str() {
                "prefix" => format!("{}{}", piece, input),
                "replace" => num_str,
                _ => format!("{}{}", input, piece),
            }
        }
        RuleParams::Extension { action, extension } => match action.as_str() {
            "set" => extension.clone(),
            "remove" => String::new(),
            "lowercase" => input.to_lowercase(),
            _ => input.to_string(),
        },
        RuleParams::DateTime { source, format, mode, separator } => {
            let date_str = match source.as_str() {
                "created" => Some(millis_fmt(ctx.created_ms, format)),
                "modified" => Some(millis_fmt(ctx.modified_ms, format)),
                "exif" => ctx.metadata.get(ctx.path).exif_date(format),
                _ => None,
            };
            match date_str {
                Some(d) => inject(input, &d, mode, separator),
                None => input.to_string(),
            }
        }
        RuleParams::Metadata { field, mode, separator, fallback } => {
            let meta = ctx.metadata.get(ctx.path);
            let value = match field.as_str() {
                "exif_camera" => meta.exif_camera_model.clone(),
                "exif_size" => meta
                    .exif_width
                    .and_then(|w| meta.exif_height.map(|h| format!("{}x{}", w, h))),
                "artist" => meta.track_artist.clone(),
                "title" => meta.track_title.clone(),
                "album" => meta.track_album.clone(),
                "track" => meta.track_number.map(|n| format!("{:02}", n)),
                _ => None,
            }
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| fallback.clone());
            inject(input, &value, mode, separator)
        }
        RuleParams::Template { template } => render_template(template, input, ctx),
        RuleParams::Cleanup {
            trim, collapse_spaces, space_replacement, strip_forbidden, strip_accents, normalize_nfc,
        } => {
            let mut s = input.to_string();
            if *strip_accents {
                s = strip_accents_fn(&s);
            }
            if *normalize_nfc {
                s = s.nfc().collect();
            }
            if *strip_forbidden {
                s = s
                    .chars()
                    .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
                    .filter(|c| !c.is_control())
                    .collect();
            }
            if *collapse_spaces {
                while s.contains("  ") {
                    s = s.replace("  ", " ");
                }
            }
            if *trim {
                s = s.trim().to_string();
            }
            if !space_replacement.is_empty() {
                s = s.replace(' ', space_replacement);
            }
            s
        }
        RuleParams::Swap { delimiter } => {
            let parts: Vec<&str> = input.splitn(2, delimiter.as_str()).collect();
            if parts.len() == 2 {
                format!("{}{}{}", parts[1].trim(), delimiter, parts[0].trim())
            } else {
                input.to_string()
            }
        }
        RuleParams::ImportList { mapping, ordered_names } => {
            if let Some((_, new)) = mapping.iter().find(|(old, _)| old == input) {
                new.clone()
            } else if let Some(new) = ordered_names.get(ctx.index.saturating_sub(1)) {
                new.clone()
            } else {
                input.to_string()
            }
        }
        RuleParams::Hash { algo, length, keep_extension } => {
            let ext = if *keep_extension {
                // Recover original extension from the original name
                ctx.original_name
                    .rfind('.')
                    .filter(|&i| i > 0)
                    .map(|i| ctx.original_name[i + 1..].to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let hash = match algo.as_str() {
                "md5" => crate::core::metadata::file_hash(ctx.path, crate::core::metadata::HashAlgo::Md5).unwrap_or_default(),
                "sha1" => crate::core::metadata::file_hash(ctx.path, crate::core::metadata::HashAlgo::Sha1).unwrap_or_default(),
                "uuid" => uuid::Uuid::new_v4().to_string(),
                _ => String::new(),
            };
            let hash = if *length > 0 && *length <= hash.len() { hash[..*length].to_string() } else { hash };
            if ext.is_empty() { hash } else { format!("{}.{}", hash, ext) }
        }
    }
}

fn apply_replace(input: &str, find: &str, replace_with: &str, case_sensitive: bool, occurrence: &str, use_regex: bool, ctx: &RuleContext) -> String {
    if use_regex {
        let pattern = if case_sensitive { find.to_string() } else { format!("(?i){}", find) };
        let replacement = replace_with.replace("$$", "$");
        return match ctx.regex_cache.get(&pattern) {
            Some(re) => re.replace_all(input, replacement.as_str()).into_owned(),
            None => input.to_string(),
        };
    }
    if find.is_empty() {
        return input.to_string();
    }
    match occurrence {
        "first" => replace_nth(input, find, replace_with, 0, case_sensitive),
        "last" => {
            let count = count_occurrences(input, find, case_sensitive);
            if count == 0 { input.to_string() } else { replace_nth(input, find, replace_with, count - 1, case_sensitive) }
        }
        _ => {
            if case_sensitive {
                input.replace(find, replace_with)
            } else {
                case_insensitive_replace_all(input, find, replace_with)
            }
        }
    }
}

fn count_occurrences(haystack: &str, needle: &str, case_sensitive: bool) -> usize {
    let (h, n) = if case_sensitive { (haystack.to_string(), needle.to_string()) } else { (haystack.to_lowercase(), needle.to_lowercase()) };
    if n.is_empty() { return 0; }
    h.matches(&n).count()
}

fn replace_nth(haystack: &str, needle: &str, replacement: &str, nth: usize, case_sensitive: bool) -> String {
    let (h, n) = if case_sensitive { (haystack.to_string(), needle.to_string()) } else { (haystack.to_lowercase(), needle.to_lowercase()) };
    if n.is_empty() { return haystack.to_string(); }
    let mut start = 0usize;
    let mut seen = 0usize;
    while let Some(pos) = h[start..].find(&n) {
        let abs = start + pos;
        if seen == nth {
            let mut out = String::with_capacity(haystack.len());
            out.push_str(&haystack[..abs]);
            out.push_str(replacement);
            out.push_str(&haystack[abs + needle.len()..]);
            return out;
        }
        seen += 1;
        start = abs + n.len();
    }
    haystack.to_string()
}

fn case_insensitive_replace_all(haystack: &str, needle: &str, replacement: &str) -> String {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    if n.is_empty() { return haystack.to_string(); }
    let mut out = String::with_capacity(haystack.len());
    let mut rest = haystack;
    let mut consumed = 0usize;
    while let Some(pos) = h[consumed..].find(&n) {
        let abs = consumed + pos;
        out.push_str(&rest[..abs - consumed]);
        out.push_str(replacement);
        let skip = rest[abs - consumed..].chars().take(n.chars().count());
        let mut skipped = 0usize;
        for c in skip {
            skipped += c.len_utf8();
        }
        rest = &rest[abs - consumed + skipped..];
        consumed = abs + skipped;
    }
    out.push_str(rest);
    out
}

fn apply_insert(input: &str, text: &str, at: &str, position: usize, from_right: bool) -> String {
    let graphemes: Vec<&str> = input.graphemes(true).collect();
    let idx = match at {
        "start" => 0,
        "end" => graphemes.len(),
        _ => {
            let p = position.max(1);
            if from_right { graphemes.len().saturating_sub(p - 1).min(graphemes.len()) } else { (p - 1).min(graphemes.len()) }
        }
    };
    let mut out = String::new();
    for (i, g) in graphemes.iter().enumerate() {
        if i == idx {
            out.push_str(text);
        }
        out.push_str(g);
    }
    if idx >= graphemes.len() {
        out.push_str(text);
    }
    out
}

fn apply_remove(input: &str, mode: &str, count: usize, from_right: bool, start: usize, end: usize) -> String {
    let graphemes: Vec<&str> = input.graphemes(true).collect();
    match mode {
        "digits" => input.chars().filter(|c| !c.is_ascii_digit()).collect(),
        "symbols" => input.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect(),
        "spaces" => input.chars().filter(|c| *c != ' ').collect(),
        "brackets" => {
            // Remove (…), […], {…} groups including the delimiters
            let mut out = String::new();
            let mut depth: Option<char> = None;
            for c in input.chars() {
                match depth {
                    Some(open) => {
                        let close = match open { '(' => ')', '[' => ']', '{' => '}', _ => ')' };
                        if c == close { depth = None; }
                    }
                    None => match c {
                        '(' | '[' | '{' => depth = Some(c),
                        _ => out.push(c),
                    },
                }
            }
            out
        }
        _ => {
            // chars mode: range or count, grapheme-safe
            let len = graphemes.len();
            let s = if start > 0 { (start - 1).min(len) } else { 0 };
            let e = if end > 0 { end.min(len) } else { len };
            let range: Vec<usize> = if count > 0 {
                if from_right {
                    let s2 = len.saturating_sub(count);
                    (s2..len).collect()
                } else {
                    (s..(s + count).min(len)).collect()
                }
            } else {
                (s..e).collect()
            };
            let mut out = String::new();
            for (i, g) in graphemes.iter().enumerate() {
                if !range.contains(&i) {
                    out.push_str(g);
                }
            }
            out
        }
    }
}

fn apply_case(input: &str, style: &str) -> String {
    match style {
        "upper" => input.to_uppercase(),
        "lower" => input.to_lowercase(),
        "title" => input
            .split(' ')
            .map(|w| {
                let mut cs = w.chars();
                match cs.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + cs.as_str().to_lowercase().as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        "sentence" => {
            let lower = input.to_lowercase();
            let mut cs = lower.chars();
            match cs.next() {
                Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        }
        "camel" => {
            let words: Vec<String> = split_words(input);
            let mut out = String::new();
            for (i, w) in words.iter().enumerate() {
                if i == 0 {
                    out.push_str(&w.to_lowercase());
                } else {
                    let mut cs = w.chars();
                    match cs.next() {
                        Some(c) => {
                            out.extend(c.to_uppercase());
                            out.push_str(cs.as_str().to_lowercase().as_str());
                        }
                        None => {}
                    }
                }
            }
            out
        }
        "snake" => split_words(input).join("_").to_lowercase(),
        "kebab" => split_words(input).join("-").to_lowercase(),
        _ => input.to_string(),
    }
}

fn split_words(input: &str) -> Vec<String> {
    input
        .split(|c: char| c == ' ' || c == '_' || c == '-' || c == '.')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

fn strip_accents_fn(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

fn millis_fmt(ms: i64, format: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.format(format).to_string())
        .unwrap_or_default()
}

fn inject(input: &str, value: &str, mode: &str, separator: &str) -> String {
    match mode {
        "prefix" => format!("{}{}{}", value, separator, input),
        "replace" => value.to_string(),
        _ => {
            if input.is_empty() { value.to_string() } else { format!("{}{}{}", input, separator, value) }
        }
    }
}

fn render_template(template: &str, input: &str, ctx: &RuleContext) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let token = &rest[open + 1..open + close];
        out.push_str(&resolve_token(token, input, ctx));
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

fn resolve_token(token: &str, input: &str, ctx: &RuleContext) -> String {
    let (name, arg) = match token.split_once(':') {
        Some((n, a)) => (n, Some(a)),
        None => (token, None),
    };
    match name {
        "name" => input.to_string(),
        "ext" => ctx
            .original_name
            .rfind('.')
            .filter(|&i| i > 0)
            .map(|i| ctx.original_name[i + 1..].to_string())
            .unwrap_or_default(),
        "n" => {
            let pad = arg.and_then(|a| a.trim_start_matches("0").parse::<usize>().ok()).unwrap_or(3);
            format!("{:0width$}", ctx.index, width = pad)
        }
        "parent" => {
            let p = std::path::Path::new(ctx.parent);
            p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| ctx.parent.to_string())
        }
        "index" => ctx.index_in_dir.to_string(),
        "modified" => millis_fmt(ctx.modified_ms, arg.unwrap_or("%Y%m%d")),
        "created" => millis_fmt(ctx.created_ms, arg.unwrap_or("%Y%m%d")),
        "exif_date" => ctx.metadata.get(ctx.path).exif_date(arg.unwrap_or("%Y%m%d")).unwrap_or_default(),
        "camera" => ctx.metadata.get(ctx.path).exif_camera_model.clone().unwrap_or_default(),
        "artist" => ctx.metadata.get(ctx.path).track_artist.clone().unwrap_or_default(),
        "title" => ctx.metadata.get(ctx.path).track_title.clone().unwrap_or_default(),
        "album" => ctx.metadata.get(ctx.path).track_album.clone().unwrap_or_default(),
        "track" => ctx.metadata.get(ctx.path).track_number.map(|t| format!("{:02}", t)).unwrap_or_default(),
        "uuid" => uuid::Uuid::new_v4().to_string(),
        "md5" => crate::core::metadata::file_hash(ctx.path, crate::core::metadata::HashAlgo::Md5).unwrap_or_default(),
        "sha1" => crate::core::metadata::file_hash(ctx.path, crate::core::metadata::HashAlgo::Sha1).unwrap_or_default(),
        other => format!("{{{}}}", other),
    }
}
