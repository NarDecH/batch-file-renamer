//! Preset persistence: JSON files with a schema version, stored in the
//! user config directory. Shared by GUI and CLI.

use crate::core::rules::RuleSpec;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PRESET_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct PresetFile {
    pub version: u32,
    pub name: String,
    pub rules: Vec<RuleSpec>,
}

pub fn preset_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("batch-renamer")
        .join("presets")
}

fn preset_path(name: &str) -> PathBuf {
    preset_dir().join(format!("{}.json", sanitize(name)))
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn save(name: &str, rules: &[RuleSpec]) -> Result<(), String> {
    let file = PresetFile {
        version: PRESET_VERSION,
        name: name.to_string(),
        rules: rules.to_vec(),
    };
    let path = preset_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn load(name: &str) -> Result<Vec<RuleSpec>, String> {
    let path = preset_path(name);
    let content = std::fs::read_to_string(&path).map_err(|e| format!("cannot read preset '{}': {}", name, e))?;
    let file: PresetFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    if file.version > PRESET_VERSION {
        return Err(format!("preset version {} is newer than supported", file.version));
    }
    Ok(file.rules)
}

pub fn list() -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(preset_dir()) else { return Ok(out) };
    for entry in rd.flatten() {
        if let Some(stem) = entry.path().file_stem() {
            out.push(stem.to_string_lossy().into_owned());
        }
    }
    out.sort();
    Ok(out)
}
