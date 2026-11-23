//! The garden folder: `garden.json` plus `photos/`, by default `<data root>/mi-huerta/`
//! (otherwise-link's data root, `~/.otherwise` unless the user moved it, e.g. to OneDrive).
//!
//! Cloud-safe: the app holds an otherwise-link lock (`garden.json.lock.json`, heartbeat) while
//! the garden is open; open on another computer → read-only until the user takes it over.
//! Writes are atomic (temp file + rename) and refuse to overwrite a newer revision. Conflicted
//! copies made by sync clients are reported ([`Store::status`]). Save As copies the folder
//! (verified, never overwrites), Move moves it (copy, verify, then delete the original). Photos
//! are **copied** in, never moved from where the user keeps them.
use crate::model::{Garden, SCHEMA};
use otherwise_link::lock::{Holder, Lock, Scope};
use std::io;
use std::path::{Path, PathBuf};

pub const APP: &str = "mi-huerta";
pub const FILE: &str = "garden.json";
pub const PHOTOS: &str = "photos";

/// `<data root>/mi-huerta`. Not created here.
pub fn default_dir() -> PathBuf {
    otherwise_link::paths::app_dir(APP)
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadOnly {
    /// Another computer has it open (its host name).
    Elsewhere(String),
    /// Another Mi Huerta on this computer has it open.
    Here,
    /// Written by a newer Mi Huerta.
    Newer(u32),
    /// Opened for reading only (MCP server, background jobs).
    Asked,
    /// Another computer took it over while it was open here.
    TakenOver,
}

#[derive(Debug)]
pub struct Store {
    pub dir: PathBuf,
    pub garden: Garden,
    lock: Option<Lock>,
    pub read_only: Option<ReadOnly>,
    /// The revision read from disk (a save checks nobody wrote a newer one).
    loaded: u64,
}

fn bad(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

/// `garden.json` in `dir`; an empty garden if there is none yet.
pub fn load(dir: &Path) -> io::Result<Garden> {
    match std::fs::read_to_string(dir.join(FILE)) {
        Ok(s) => serde_json::from_str(&s).map_err(bad),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Garden::default()),
        Err(e) => Err(e),
    }
}

impl Store {
    /// Opens `dir` for writing (created if needed), or read-only when it is open elsewhere or
    /// newer than this version.
    pub fn open(dir: &Path) -> io::Result<Store> {
        std::fs::create_dir_all(dir)?;
        let garden = load(dir)?;
        let loaded = garden.revision;
        let mut s = Store { dir: dir.to_path_buf(), garden, lock: None, read_only: None, loaded };
        if s.garden.schema > SCHEMA {
            s.read_only = Some(ReadOnly::Newer(s.garden.schema));
            return Ok(s);
        }
        match Lock::acquire(&s.file(), APP, Scope::Process) {
            Ok(l) => s.lock = Some(l),
            Err(Holder::Elsewhere(i)) => s.read_only = Some(ReadOnly::Elsewhere(i.host)),
            Err(_) => s.read_only = Some(ReadOnly::Here),
        }
        Ok(s)
    }

    /// No lock, no writes (the MCP server, Otherwise Helper's jobs).
    pub fn open_read_only(dir: &Path) -> io::Result<Store> {
        let garden = load(dir)?;
        let loaded = garden.revision;
        Ok(Store { dir: dir.to_path_buf(), garden, lock: None, read_only: Some(ReadOnly::Asked), loaded })
    }

    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    pub fn photos_dir(&self) -> PathBuf {
        self.dir.join(PHOTOS)
    }

    pub fn writable(&self) -> bool {
        self.read_only.is_none()
    }

    /// "Use here anyway" after [`ReadOnly::Elsewhere`] / [`ReadOnly::TakenOver`].
    pub fn take_over(&mut self) -> io::Result<()> {
        if matches!(self.read_only, Some(ReadOnly::Newer(_) | ReadOnly::Asked)) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "read-only"));
        }
        self.lock = Some(Lock::take_over(&self.file(), APP, Scope::Process)?);
        self.garden = load(&self.dir)?;
        self.loaded = self.garden.revision;
        self.read_only = None;
        Ok(())
    }

    /// Keeps the lock alive (call every `otherwise_link::lock::HEARTBEAT_EVERY`); `false` when
    /// another computer took the garden over (now read-only).
    pub fn heartbeat(&mut self) -> bool {
        let Some(l) = self.lock.as_mut() else { return self.read_only.is_none() };
        if l.heartbeat().is_err() {
            self.lock = None;
            self.read_only = Some(ReadOnly::TakenOver);
            return false;
        }
        true
    }

    /// Has someone else written `garden.json` since we read it?
    pub fn changed_on_disk(&self) -> bool {
        load(&self.dir).is_ok_and(|g| g.revision != self.loaded)
    }

    /// Re-reads the file (read-only views, after an outside change).
    pub fn reload(&mut self) -> io::Result<()> {
        self.garden = load(&self.dir)?;
        self.loaded = self.garden.revision;
        Ok(())
    }

    pub fn save(&mut self) -> io::Result<()> {
        if !self.writable() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "the garden is open read-only"));
        }
        let on_disk = load(&self.dir).map(|g| g.revision).unwrap_or(0);
        if on_disk > self.loaded {
            return Err(io::Error::other("garden.json changed elsewhere; reload first"));
        }
        self.garden.schema = SCHEMA;
        self.garden.revision = self.loaded + 1;
        let json = serde_json::to_string_pretty(&self.garden).map_err(bad)? + "\n";
        otherwise_link::fsutil::write_atomic(&self.file(), json.as_bytes())?;
        self.loaded = self.garden.revision;
        Ok(())
    }

    /// Cloud provider, lock holder and conflicted copies of `garden.json`.
    pub fn status(&self) -> otherwise_link::storage::Status {
        otherwise_link::storage::status(&self.file())
    }

    /// Copies a photo from wherever the user keeps it into `photos/` (never moves it, never
    /// overwrites); returns its path relative to the garden folder.
    pub fn add_photo(&self, src: &Path, today: chrono::NaiveDate) -> io::Result<String> {
        let ext = src.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).filter(|e| PHOTO_EXTS.contains(&e.as_str())).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a photo"))?;
        std::fs::create_dir_all(self.photos_dir())?;
        let stem = format!("{today}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let name = format!("{stem}.{ext}");
        otherwise_link::storage::copy_file(src, &self.photos_dir().join(&name))?;
        Ok(format!("{PHOTOS}/{name}"))
    }

    pub fn photo_path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    /// Save As: a verified copy of the whole folder at `dest` (must not exist or be empty),
    /// which becomes the open garden; the original stays as it was.
    pub fn save_as(&mut self, dest: &Path) -> io::Result<()> {
        if self.writable() {
            self.save()?;
        }
        ensure_free(dest)?;
        otherwise_link::storage::copy_tree(&self.dir, dest)?;
        self.switch_to(dest)
    }

    /// Move: the folder goes to `dest` (verified copy, then the original is removed).
    pub fn move_to(&mut self, dest: &Path) -> io::Result<()> {
        if !self.writable() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "the garden is open read-only"));
        }
        self.save()?;
        ensure_free(dest)?;
        if let Some(l) = self.lock.take() {
            l.release();
        }
        if let Err(e) = otherwise_link::storage::move_tree(&self.dir, dest) {
            // still where it was: take the lock back
            self.lock = Lock::acquire(&self.file(), APP, Scope::Process).ok();
            return Err(e);
        }
        self.switch_to(dest)
    }

    fn switch_to(&mut self, dest: &Path) -> io::Result<()> {
        let next = Store::open(dest)?;
        if let Some(l) = self.lock.take() {
            l.release();
        }
        *self = next;
        Ok(())
    }
}

pub const PHOTO_EXTS: &[&str] = &["jpg", "jpeg", "png", "heic", "webp", "gif", "tif", "tiff"];

fn ensure_free(dest: &Path) -> io::Result<()> {
    if dest.exists() && std::fs::read_dir(dest)?.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} is not empty", dest.display())));
    }
    Ok(())
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(l) = self.lock.take() {
            l.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Plant;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mi-huerta-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn save_reload_lock_and_stale_writes() {
        let dir = tmp("save");
        let mut s = Store::open(&dir).unwrap();
        assert!(s.writable());
        assert!(dir.join("garden.json.lock.json").exists(), "the lock sits next to the file");
        s.garden.plants.push(Plant { id: "p1".into(), name: "Albahaca".into(), ..Default::default() });
        s.save().unwrap();
        assert_eq!(load(&dir).unwrap().revision, 1);
        // a second store in this process sees the lock as ours: simulate another writer instead
        let mut other = load(&dir).unwrap();
        other.revision = 5;
        std::fs::write(dir.join(FILE), serde_json::to_string(&other).unwrap()).unwrap();
        assert!(s.changed_on_disk());
        assert!(s.save().is_err(), "never overwrite a newer revision");
        s.reload().unwrap();
        s.save().unwrap();
        assert_eq!(load(&dir).unwrap().revision, 6);
        let ro = Store::open_read_only(&dir).unwrap();
        assert!(!ro.writable());
        drop(s);
        assert!(!dir.join("garden.json.lock.json").exists(), "released on close");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn newer_gardens_open_read_only() {
        let dir = tmp("newer");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), r#"{"schema": 99}"#).unwrap();
        let mut s = Store::open(&dir).unwrap();
        assert_eq!(s.read_only, Some(ReadOnly::Newer(99)));
        assert!(s.save().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn photos_are_copied_and_folders_saved_as_and_moved() {
        let base = tmp("photos");
        let outside = base.join("Mis fotos");
        std::fs::create_dir_all(&outside).unwrap();
        let photo = outside.join("tomatera.JPG");
        std::fs::write(&photo, b"jpeg bytes").unwrap();
        let mut s = Store::open(&base.join("garden")).unwrap();
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let rel = s.add_photo(&photo, day).unwrap();
        assert!(rel.starts_with("photos/2026-11-23-") && rel.ends_with(".jpg"), "{rel}");
        assert!(photo.exists(), "the user's photo stays where it was");
        assert_eq!(std::fs::read(s.photo_path(&rel)).unwrap(), b"jpeg bytes");
        assert!(s.add_photo(&outside.join("notes.txt"), day).is_err());
        s.garden.plants.push(Plant { id: "p1".into(), name: "Tomatera".into(), photos: vec![rel.clone()], ..Default::default() });
        s.save().unwrap();

        let copy = base.join("copia");
        s.save_as(&copy).unwrap();
        assert_eq!(s.dir, copy);
        assert!(base.join("garden").join(FILE).exists(), "Save As leaves the original");
        assert!(copy.join(&rel).exists());
        assert!(!copy.join("garden.json.lock.json").exists() || s.writable());

        let moved = base.join("OneDrive/Mi Huerta");
        s.move_to(&moved).unwrap();
        assert_eq!(s.dir, moved);
        assert!(!copy.join(FILE).exists(), "moved, not copied");
        assert_eq!(s.garden.plants[0].name, "Tomatera");
        assert!(moved.join(&rel).exists());
        // never into a folder with things in it
        assert!(s.save_as(&base.join("garden")).is_err());
        drop(s);
        let _ = std::fs::remove_dir_all(&base);
    }
}
