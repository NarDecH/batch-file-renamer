//! Process-wide compiled-regex cache. Compiling a regex costs microseconds to
//! milliseconds; a 100k-file preview must compile each pattern exactly once.

use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct RegexCache {
    cache: Mutex<HashMap<String, std::sync::Arc<Regex>>>,
}

impl RegexCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get (or compile) the pattern. Invalid patterns return None.
    pub fn get(&self, pattern: &str) -> Option<std::sync::Arc<Regex>> {
        let mut guard = self.cache.lock().unwrap();
        if let Some(re) = guard.get(pattern) {
            return Some(re.clone());
        }
        match Regex::new(pattern) {
            Ok(re) => {
                let arc = std::sync::Arc::new(re);
                guard.insert(pattern.to_string(), arc.clone());
                Some(arc)
            }
            Err(_) => None,
        }
    }
}
