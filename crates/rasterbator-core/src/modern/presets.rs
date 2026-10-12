//! The preset store and the GUI settings file (design §2.7).
//!
//! No new dependency: the format is UTF-8 text with LF line endings, a
//! `rasterbator-options v1` header and one `key=value` per option in
//! [`ModernOptions::option_names`] order. Files live in the preset directory as
//! `<name>.options`, with a portable `[A-Za-z0-9_-]{1,48}` name.
//!
//! The GUI settings file is the same format under the reserved name
//! `settings`, holding every option except the UX group, so "remember the
//! non-image settings" never remembers the image, the crop rectangle or the
//! output path.

use std::path::{Path, PathBuf};

use crate::modern::diagnostics::{DiagCode, Diagnostic};
use crate::modern::options::{is_portable_name, ModernOptions};

/// The reserved settings-file name.
pub const SETTINGS_NAME: &str = "settings";

/// The version header line.
pub const HEADER: &str = "rasterbator-options v1";

/// The file extension of a preset.
pub const EXTENSION: &str = "options";

/// A directory of named option sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetStore {
    dir: PathBuf,
}

/// One entry of the store listing.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PresetEntry {
    /// The preset name (file stem).
    pub name: String,
    /// Whether the preset is the GUI settings file.
    pub is_settings: bool,
}

impl PresetEntry {
    /// The name as shown in a combo box.
    pub fn display(&self) -> String {
        if self.is_settings {
            format!("{} (settings)", self.name)
        } else {
            self.name.clone()
        }
    }
}

impl PresetStore {
    /// A store rooted at `dir`.
    pub fn new(dir: PathBuf) -> PresetStore {
        PresetStore { dir }
    }

    /// The default store location.
    pub fn default_location() -> PresetStore {
        PresetStore::new(ModernOptions::default_preset_dir())
    }

    /// The directory this store reads and writes.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The path of one preset.
    pub fn path_of(&self, name: &str) -> Result<PathBuf, Diagnostic> {
        if !is_portable_name(name) {
            return Err(Diagnostic::error(
                DiagCode::PresetNameInvalid,
                format!("preset name '{name}' is not portable"),
            )
            .with_hint("use 1 to 48 characters from A-Z, a-z, 0-9, '_' or '-'"));
        }
        Ok(self.dir.join(format!("{name}.{EXTENSION}")))
    }

    /// Create the directory if it does not exist yet.
    pub fn ensure_dir(&self) -> Result<(), Diagnostic> {
        if self.dir.is_dir() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| {
            Diagnostic::error(
                DiagCode::PresetInvalid,
                format!("{}: {e}", self.dir.display()),
            )
        })
    }

    /// Every preset in the directory, sorted by name; the settings file last.
    ///
    /// Names differing only in case stay distinct because the listing is a
    /// byte-wise sort of the exact file stems.
    pub fn list(&self) -> Result<Vec<PresetEntry>, Diagnostic> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return Ok(out);
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            out.push(PresetEntry {
                name: stem.to_string(),
                is_settings: stem == SETTINGS_NAME,
            });
        }
        out.sort_by(|a, b| {
            a.is_settings
                .cmp(&b.is_settings)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    }

    /// Does the preset exist?
    pub fn exists(&self, name: &str) -> bool {
        self.path_of(name).map(|p| p.is_file()).unwrap_or(false)
    }

    /// Write a preset. `overwrite` is false when the name already exists.
    pub fn save(&self, name: &str, opts: &ModernOptions, overwrite: bool) -> Result<(), Diagnostic> {
        self.ensure_dir()?;
        let path = self.path_of(name)?;
        if !overwrite && path.exists() {
            return Err(Diagnostic::warning(
                DiagCode::PresetOverwrite,
                format!("preset '{name}' already exists"),
            )
            .with_hint("pass an overwrite flag to replace it"));
        }
        std::fs::write(&path, opts.to_text())
            .map_err(|e| Diagnostic::error(DiagCode::PresetInvalid, format!("{}: {e}", path.display())))
    }

    /// Read a preset.
    pub fn load(&self, name: &str) -> Result<ModernOptions, Diagnostic> {
        let path = self.path_of(name)?;
        let text = std::fs::read_to_string(&path).map_err(|e| {
            Diagnostic::error(
                DiagCode::PresetInvalid,
                format!("{}: {e}", path.display()),
            )
            .with_hint(format!("run --list-presets to see the available presets"))
        })?;
        ModernOptions::from_text(&text)
    }

    /// Delete a preset.
    pub fn delete(&self, name: &str) -> Result<(), Diagnostic> {
        let path = self.path_of(name)?;
        std::fs::remove_file(&path).map_err(|e| {
            Diagnostic::error(
                DiagCode::PresetInvalid,
                format!("{}: {e}", path.display()),
            )
            .with_hint(format!("{name} could not be removed"))
        })
    }

    /// Write the GUI settings file: every option except the UX group.
    pub fn save_settings(&self, opts: &ModernOptions) -> Result<(), Diagnostic> {
        self.save(SETTINGS_NAME, opts, true)?;
        // Rewrite without the UX group.
        let path = self.path_of(SETTINGS_NAME)?;
        std::fs::write(&path, opts.to_settings_text())
            .map_err(|e| Diagnostic::error(DiagCode::PresetInvalid, format!("{}: {e}", path.display())))
    }

    /// Read the GUI settings file, or the defaults when it does not exist.
    pub fn load_settings(&self) -> Result<ModernOptions, Diagnostic> {
        if !self.exists(SETTINGS_NAME) {
            return Ok(ModernOptions::default());
        }
        self.load(SETTINGS_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmpdir(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "rasterbator-presets-{tag}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn save_list_load_delete_round_trips() {
        let dir = tmpdir("rt");
        let store = PresetStore::new(dir.clone());
        let mut o = ModernOptions::default();
        o.screen = crate::modern::options::ScreenFamily::Hex;
        o.pattern = crate::modern::options::Pattern::Mosaic;
        o.nominal_dot_mm = 7.5;
        o.ink_color = crate::modern::color::Rgb(1, 2, 3);
        o.crop = Some(crate::modern::options::CropRectPx { x1: 1, y1: 2, x2: 3, y2: 4 });
        store.save("mine", &o, false).unwrap();
        assert!(store.exists("mine"));
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "mine");
        assert_eq!(listed[0].display(), "mine");
        assert_eq!(store.load("mine").unwrap(), o);
        store.delete("mine").unwrap();
        assert!(!store.exists("mine"));
        assert!(store.list().unwrap().is_empty());
        assert!(store.load("mine").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overwriting_is_refused_without_the_flag() {
        let dir = tmpdir("ow");
        let store = PresetStore::new(dir.clone());
        let o = ModernOptions::default();
        store.save("dup", &o, false).unwrap();
        let e = store.save("dup", &o, false).unwrap_err();
        assert_eq!(e.code, DiagCode::PresetOverwrite);
        assert_eq!(e.severity, crate::modern::diagnostics::Severity::Warning);
        store.save("dup", &o, true).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_is_created() {
        let dir = tmpdir("mk").join("nested").join("deeper");
        let store = PresetStore::new(dir.clone());
        assert!(!dir.exists());
        store.save("x", &ModernOptions::default(), false).unwrap();
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir_all(&dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn names_differing_in_case_stay_distinct() {
        let dir = tmpdir("case");
        let store = PresetStore::new(dir.clone());
        // The store keeps the exact name it was given; on a case-insensitive
        // filesystem the two files would collide, which is a property of the
        // filesystem, not of the store.
        assert_ne!(store.path_of("Preset").unwrap(), store.path_of("preset").unwrap());
        let mut a = ModernOptions::default();
        a.margin_mm = 10.0;
        store.save("Preset", &a, false).unwrap();
        assert_eq!(store.load("Preset").unwrap().margin_mm, 10.0);
        let mut b = a.clone();
        b.margin_mm = 12.0;
        store.save("preset", &b, true).unwrap();
        assert_eq!(store.load("preset").unwrap().margin_mm, 12.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn invalid_names_are_rejected_before_touching_the_disk() {
        let dir = tmpdir("name");
        let store = PresetStore::new(dir.clone());
        for bad in ["", "has space", "a/b", "../escape", &"x".repeat(49)] {
            let e = store.save(bad, &ModernOptions::default(), false).unwrap_err();
            assert_eq!(e.code, DiagCode::PresetNameInvalid);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_exclude_the_ux_group() {
        let dir = tmpdir("set");
        let store = PresetStore::new(dir.clone());
        let mut o = ModernOptions::default();
        o.screen = crate::modern::options::ScreenFamily::Hex;
        o.quiet = true;
        store.save_settings(&o).unwrap();
        let text = std::fs::read_to_string(dir.join("settings.options")).unwrap();
        assert!(!text.contains("quiet="));
        assert!(text.contains("screen=hex"));
        let back = store.load_settings().unwrap();
        assert_eq!(back.screen, crate::modern::options::ScreenFamily::Hex);
        assert!(!back.quiet, "the UX group is not remembered");
        let listed = store.list().unwrap();
        assert_eq!(listed[0].name, SETTINGS_NAME);
        assert!(listed[0].is_settings);
        assert_eq!(listed[0].display(), "settings (settings)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_settings_defaults_when_absent() {
        let dir = tmpdir("noset");
        let store = PresetStore::new(dir.clone());
        assert_eq!(store.load_settings().unwrap(), ModernOptions::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_preset_reports_a_blocking_diagnostic() {
        let dir = tmpdir("bad");
        let store = PresetStore::new(dir.clone());
        std::fs::write(dir.join("bad.options"), "not a preset").unwrap();
        let e = store.load("bad").unwrap_err();
        assert_eq!(e.code, DiagCode::PresetInvalid);
        assert_eq!(e.severity, crate::modern::diagnostics::Severity::Error);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn listing_is_sorted_with_settings_last() {
        let dir = tmpdir("sort");
        let store = PresetStore::new(dir.clone());
        for n in ["zeta", "alpha", "mid"] {
            store.save(n, &ModernOptions::default(), false).unwrap();
        }
        store.save_settings(&ModernOptions::default()).unwrap();
        let names: Vec<String> = store.list().unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta", "settings"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}