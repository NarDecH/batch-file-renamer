//! CSV export of preview rows (RFC 4180 escaping, Excel-friendly UTF-8 BOM).

/// Escape one CSV field: quote only when needed (comma, quote, CR/LF).
fn escape_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn iso_date(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Row shape received from the UI (camelCase via serde).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRow {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub old_name: String,
    #[serde(default)]
    pub new_name: String,
    #[serde(default)]
    pub parent: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub modified_ms: i64,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub message: Option<String>,
}

/// Build the full CSV text (with BOM) for a list of rows.
pub fn rows_to_csv(rows: &[ExportRow]) -> String {
    let mut out = String::with_capacity(rows.len() * 64 + 64);
    out.push_str("status,old_name,new_name,folder,size,modified,message\r\n");
    for r in rows {
        let line = [
            escape_field(&r.status),
            escape_field(&r.old_name),
            escape_field(&r.new_name),
            escape_field(&r.parent),
            if r.is_dir { String::new() } else { r.size.to_string() },
            escape_field(&iso_date(r.modified_ms)),
            escape_field(r.message.as_deref().unwrap_or("")),
        ]
        .join(",");
        out.push_str(&line);
        out.push_str("\r\n");
    }
    // BOM makes Excel open UTF-8 correctly
    format!("\u{feff}{}", out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: &str, old: &str, new: &str, msg: Option<&str>) -> ExportRow {
        ExportRow {
            status: status.into(),
            old_name: old.into(),
            new_name: new.into(),
            parent: r#"D:\Trip, Asia"#.into(),
            size: 1234,
            modified_ms: 1_700_000_000_000,
            is_dir: false,
            message: msg.map(|m| m.into()),
        }
    }

    #[test]
    fn header_and_bom() {
        let csv = rows_to_csv(&[]);
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("status,old_name,new_name,folder,size,modified,message"));
    }

    #[test]
    fn quotes_fields_with_comma() {
        let csv = rows_to_csv(&[row("ready", "a.jpg", "b.jpg", None)]);
        assert!(csv.contains(r#""D:\Trip, Asia""#));
    }

    #[test]
    fn escapes_quotes_and_newlines_in_message() {
        let csv = rows_to_csv(&[row("error", "a.jpg", "b.jpg", Some("bad \"name\"\nline2"))]);
        // real newline inside a quoted field
        assert!(csv.contains("\"bad \"\"name\"\"\nline2\""));
    }

    #[test]
    fn dirs_have_no_size() {
        let mut r = row("ready", "dir", "dir", None);
        r.is_dir = true;
        let csv = rows_to_csv(&[r]);
        let line = csv.lines().nth(1).unwrap();
        // size column must be empty for directories
        assert!(line.contains(",,"), "line: {}", line);
    }

    #[test]
    fn modified_is_iso() {
        let csv = rows_to_csv(&[row("ready", "a.jpg", "b.jpg", None)]);
        // 1700000000 = 2023-11-14 in UTC
        assert!(csv.contains("2023-11-14"), "line: {}", csv);
    }
}
