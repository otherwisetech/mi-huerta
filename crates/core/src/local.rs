//! Machine-local settings (not data, never synced): the language, the theme, and where the
//! garden folder is when the user chose one. In the platform's config folder:
//! `~/Library/Application Support/Mi Huerta/settings.json`, `%APPDATA%\Mi Huerta\…`,
//! `$XDG_CONFIG_HOME/mi-huerta/…`.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    pub language: Option<String>,
    /// "light", "dark", anything else follows the system.
    pub theme: Option<String>,
    /// A garden folder the user chose (Save As / Move / Open). `None`: the default, resolved
    /// fresh every time so it follows the data root.
    pub garden_dir: Option<PathBuf>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn config_dir() -> PathBuf {
    let home = otherwise_link::paths::home();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Mi Huerta")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| home.join("AppData/Roaming")).join("Mi Huerta")
    } else {
        std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".config")).join("mi-huerta")
    }
}

impl Local {
    pub fn load_from(file: &Path) -> Local {
        std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save_to(&self, file: &Path) -> std::io::Result<()> {
        otherwise_link::fsutil::write_atomic(file, (serde_json::to_string_pretty(self).unwrap_or_default() + "\n").as_bytes())
    }

    pub fn file() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Local {
        Local::load_from(&Local::file())
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&Local::file())
    }

    pub fn update(f: impl FnOnce(&mut Local)) {
        let mut l = Local::load();
        f(&mut l);
        let _ = l.save();
    }

    /// The garden folder to open.
    pub fn garden_dir(&self) -> PathBuf {
        self.garden_dir.clone().unwrap_or_else(crate::store::default_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_default_dir() {
        let dir = std::env::temp_dir().join(format!("mi-huerta-local-{}", std::process::id()));
        let file = dir.join("settings.json");
        assert_eq!(Local::load_from(&file), Local::default());
        let l = Local { language: Some("pt".into()), garden_dir: Some(dir.join("g")), ..Default::default() };
        l.save_to(&file).unwrap();
        assert_eq!(Local::load_from(&file), l);
        assert_eq!(l.garden_dir(), dir.join("g"));
        assert!(Local::default().garden_dir().ends_with("mi-huerta"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
