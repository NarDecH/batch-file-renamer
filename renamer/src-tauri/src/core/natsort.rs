//! Natural sort (human-aware ordering): digit runs compare numerically,
//! everything else lexicographically (case-insensitive tie-break).

/// Split a string into digit / non-digit chunks, e.g. "file10a" -> ["file", "10", "a"].
pub fn natural_key(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_digits = false;
    for (i, ch) in s.char_indices() {
        let d = ch.is_ascii_digit();
        if d != in_digits && i > start {
            parts.push(&s[start..i]);
            start = i;
        }
        in_digits = d;
    }
    parts.push(&s[start..]);
    parts
}

/// Compare two strings in natural order.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let ka = natural_key(a);
    let kb = natural_key(b);
    for (pa, pb) in ka.iter().zip(kb.iter()) {
        let a_digits = pa.chars().next().is_some_and(|c| c.is_ascii_digit());
        let b_digits = pb.chars().next().is_some_and(|c| c.is_ascii_digit());
        let ord = match (a_digits, b_digits) {
            (true, true) => {
                // Compare numeric value first (strip leading zeros), then length as tie-break
                let na = pa.trim_start_matches('0');
                let nb = pb.trim_start_matches('0');
                match na.len().cmp(&nb.len()).then_with(|| na.cmp(nb)) {
                    Ordering::Equal => {}
                    o => return o,
                }
                Ordering::Equal
            }
            _ => pa.to_lowercase().cmp(&pb.to_lowercase()),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    ka.len().cmp(&kb.len()).then_with(|| a.cmp(b))
}

/// Sort entries by their natural path order (parent dir, then file name).
pub fn sort_entries_by_path(entries: &mut [super::models::FileEntry]) {
    entries.sort_by(|a, b| {
        let pa = a.path.to_string_lossy();
        let pb = b.path.to_string_lossy();
        natural_cmp(&pa, &pb)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order_digits() {
        let mut names = vec!["ep10", "ep2", "ep1"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, vec!["ep1", "ep2", "ep10"]);
    }

    #[test]
    fn leading_zeros() {
        // Same numeric value: stable lexicographic tie-break
        assert_eq!(natural_cmp("a01", "a1"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("a1", "a10"), std::cmp::Ordering::Less);
    }

    #[test]
    fn mixed_text_digits() {
        let mut names = vec!["file10a", "file2b", "file1a"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, vec!["file1a", "file2b", "file10a"]);
    }
}
