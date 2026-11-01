//! Preset files: `presets/<name>.preset` at the workspace root, plain text (see `keebs_core::preset`).

use keebs_core::{ParamValues, preset};
use std::path::PathBuf;

const EXT: &str = "preset";

/// The built-in preset: every parameter at its default. Always listed first; never a file.
pub const DEFAULT: &str = "Default";

/// Kept in the repo so presets travel with the code and diff readably.
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets")
}

fn path(name: &str) -> PathBuf {
    dir().join(format!("{name}.{EXT}"))
}

/// A usable preset name, or why not.
pub fn check_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("give the preset a name".into());
    }
    if name.eq_ignore_ascii_case(DEFAULT) {
        return Err(format!("'{DEFAULT}' is built in; pick another name"));
    }
    if name.starts_with('.') || name.contains(['/', '\\', ':']) {
        return Err("a name can't start with '.' or contain / \\ :".into());
    }
    Ok(name)
}

/// The built-in default, then saved preset names sorted case-insensitively.
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXT))
        .filter_map(|p| Some(p.file_stem()?.to_str()?.to_string()))
        .filter(|n| !n.eq_ignore_ascii_case(DEFAULT))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.insert(0, DEFAULT.to_string());
    names
}

pub fn exists(name: &str) -> bool {
    path(name).is_file()
}

/// Load a preset; also returns warnings about lines that were skipped or clamped.
pub fn load(name: &str) -> Result<(ParamValues, Vec<String>), String> {
    if name == DEFAULT {
        return Ok((ParamValues::default(), Vec::new()));
    }
    let text = std::fs::read_to_string(path(name)).map_err(|e| format!("can't read '{name}': {e}"))?;
    Ok(preset::from_text(&text))
}

/// Write via a temporary file so a failed save never leaves a half-written preset.
pub fn save(name: &str, p: &ParamValues) -> Result<(), String> {
    let name = check_name(name)?;
    std::fs::create_dir_all(dir()).map_err(|e| format!("can't create presets folder: {e}"))?;
    let tmp = dir().join(format!(".{name}.{EXT}.tmp"));
    std::fs::write(&tmp, preset::to_text(p)).map_err(|e| format!("can't write '{name}': {e}"))?;
    std::fs::rename(&tmp, path(name)).map_err(|e| format!("can't write '{name}': {e}"))
}

pub fn delete(name: &str) -> Result<(), String> {
    std::fs::remove_file(path(name)).map_err(|e| format!("can't delete '{name}': {e}"))
}
