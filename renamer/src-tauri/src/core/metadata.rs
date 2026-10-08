//! Lazy metadata access: EXIF (photos) and music tags.
//!
//! Values are read on demand - only when a rule actually needs them - and
//! cached per file path for the lifetime of the process.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct FileMetadata {
    /// EXIF DateTimeOriginal, e.g. "2024:03:15 14:30:22"
    pub exif_date_original: Option<String>,
    pub exif_camera_model: Option<String>,
    pub exif_width: Option<u32>,
    pub exif_height: Option<u32>,
    /// Audio tags
    pub track_artist: Option<String>,
    pub track_title: Option<String>,
    pub track_album: Option<String>,
    pub track_number: Option<u32>,
}

impl FileMetadata {
    /// EXIF date formatted as `format` (chrono strftime); falls back to file mtime.
    pub fn exif_date(&self, format: &str) -> Option<String> {
        let raw = self.exif_date_original.as_deref()?;
        let dt = chrono::NaiveDateTime::parse_from_str(raw, "%Y:%m:%d %H:%M:%S")
            .ok()
            .or_else(|| chrono::NaiveDate::parse_from_str(raw, "%Y:%m:%d").ok().map(|d| d.and_hms_opt(0, 0, 0).unwrap()))?;
        Some(dt.format(format).to_string())
    }
}

/// Thread-safe lazy metadata cache.
#[derive(Default)]
pub struct MetadataCache {
    cache: Mutex<HashMap<PathBuf, FileMetadata>>,
}

impl MetadataCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, path: &Path) -> FileMetadata {
        let mut guard = self.cache.lock().unwrap();
        if let Some(m) = guard.get(path) {
            return m.clone();
        }
        let m = read_metadata(path);
        guard.insert(path.to_path_buf(), m.clone());
        m
    }
}

fn read_metadata(path: &Path) -> FileMetadata {
    let mut m = FileMetadata::default();
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return m;
    };
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "tif" | "tiff" | "heic" | "png" | "webp" => read_exif(path, &mut m),
        "mp3" | "flac" | "ogg" | "opus" | "m4a" | "wav" | "aac" => read_tags(path, &mut m),
        _ => {}
    }
    m
}

fn read_exif(path: &Path, m: &mut FileMetadata) {
    let Ok(file) = std::fs::File::open(path) else { return };
    let Ok(exif) = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)) else { return };

    if let Some(tag) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        m.exif_date_original = Some(tag.display_value().to_string().trim().to_string());
    }
    if let Some(tag) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        m.exif_camera_model = Some(tag.display_value().to_string().trim().to_string());
    }
    if let Some(tag) = exif.get_field(exif::Tag::ImageWidth, exif::In::PRIMARY) {
        m.exif_width = tag.value.get_uint(0);
    }
    if let Some(tag) = exif.get_field(exif::Tag::ImageLength, exif::In::PRIMARY) {
        m.exif_height = tag.value.get_uint(0);
    }
}

fn read_tags(path: &Path, m: &mut FileMetadata) {
    use lofty::file::TaggedFileExt;
    use lofty::tag::Accessor;
    let Ok(mut probe) = lofty::probe::Probe::open(path) else { return };
    let Ok(tagged) = probe.read() else { return };
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    if let Some(tag) = tag {
        m.track_artist = tag.artist().map(|s| s.to_string());
        m.track_title = tag.title().map(|s| s.to_string());
        m.track_album = tag.album().map(|s| s.to_string());
        m.track_number = tag.track().map(|t| t as u32);
    }
}

/// Compute a hash of the file contents (used by Hash/Random rule).
pub fn file_hash(path: &Path, algo: HashAlgo) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    match algo {
        HashAlgo::Md5 => {
            let mut ctx = md5::Context::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = file.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                ctx.consume(&buf[..n]);
            }
            Ok(format!("{:x}", ctx.compute()))
        }
        HashAlgo::Sha1 => {
            use sha1::Digest;
            let mut hasher = sha1::Sha1::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = file.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            Ok(hex_encode(&hasher.finalize()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgo {
    Md5,
    Sha1,
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
