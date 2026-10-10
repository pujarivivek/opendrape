//! A copy of unsaved work, written when OpenDrape is quit in a way that can't ask first (from
//! the Dock, or at logout or shutdown, where macOS ends the app without a close request) and
//! offered back the next time it starts.

use opendrape_core::Project;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Recovery {
    dir: Option<PathBuf>,
}

impl Recovery {
    /// Keeps the copy in `dir`; `None` keeps no copy at all (tests, or a second running copy of
    /// OpenDrape, which must not overwrite the first one's).
    pub fn new(dir: Option<&Path>) -> Self {
        Self {
            dir: dir.map(Path::to_path_buf),
        }
    }

    /// OpenDrape's settings folder, next to the graphics settings.
    pub fn default_location() -> Self {
        Self::new(crate::gpu::config_dir().as_deref())
    }

    fn copy(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("recovery.odp"))
    }

    fn origin(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("recovery-origin.txt"))
    }

    /// Writes `project`, and the file it came from when it has one. OpenDrape is closing and
    /// can't show anything, so failures are only logged.
    ///
    /// The note of an earlier copy is deleted first and the new one written last, so that
    /// whatever fails in between leaves a copy with no note (it comes back untitled), never a
    /// copy that names the file of some other project.
    pub fn write(&self, project: &Project, from: Option<&Path>) {
        let (Some(dir), Some(copy), Some(origin)) = (&self.dir, self.copy(), self.origin()) else {
            return;
        };
        if let Err(e) = std::fs::create_dir_all(dir) {
            crate::startup_log::stage(format_args!("recovery: no folder {dir:?}: {e}"));
            return;
        }
        delete(&origin);
        if let Err(e) = opendrape_io::save(project, &copy) {
            crate::startup_log::stage(format_args!("recovery: could not write {copy:?}: {e}"));
            return;
        }
        // A path that is not UTF-8 can't be written as text, so it is not remembered.
        if let Some(path) = from.and_then(Path::to_str)
            && let Err(e) = std::fs::write(&origin, path)
        {
            crate::startup_log::stage(format_args!("recovery: could not note the file: {e}"));
        }
    }

    /// The waiting copy and the file it came from, if there is a copy that opens. A copy that
    /// is damaged or can't be read is deleted; one made by a newer OpenDrape is kept for when
    /// that is installed again, and not offered. A note that is empty, or doesn't hold a full
    /// path (a note cut short, or edited by hand), counts as no file.
    pub fn take(&self) -> Option<(Project, Option<PathBuf>)> {
        let copy = self.copy().filter(|c| c.exists())?;
        match opendrape_io::load(&copy) {
            Ok(project) => {
                let from = self
                    .origin()
                    .and_then(|o| std::fs::read_to_string(o).ok())
                    .map(|note| PathBuf::from(note.trim()))
                    .filter(|path| path.is_absolute());
                Some((project, from))
            }
            Err(e @ opendrape_io::OdpError::NewerVersion { .. }) => {
                crate::startup_log::stage(format_args!("recovery: keeping {copy:?}: {e}"));
                None
            }
            Err(e) => {
                crate::startup_log::stage(format_args!("recovery: dropping {copy:?}: {e}"));
                self.discard();
                None
            }
        }
    }

    /// Deletes the copy.
    pub fn discard(&self) {
        for file in [self.copy(), self.origin()].into_iter().flatten() {
            delete(&file);
        }
    }
}

/// Deletes `file`. One that is already gone is fine; any other failure is logged.
fn delete(file: &Path) {
    if let Err(e) = std::fs::remove_file(file)
        && e.kind() != ErrorKind::NotFound
    {
        crate::startup_log::stage(format_args!("recovery: could not delete {file:?}: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, PieceId, Point2};

    fn project() -> Project {
        let mut p = Project::new();
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            500.0,
        ));
        p
    }

    /// A full path on every system (a note must hold one to be believed).
    fn skirt() -> PathBuf {
        std::env::temp_dir().join("work").join("skirt.odp")
    }

    #[test]
    fn writes_and_takes_back_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        assert!(r.take().is_none());
        r.write(&project(), Some(&skirt()));
        let (back, from) = r.take().unwrap();
        assert_eq!(back, project());
        assert_eq!(from, Some(skirt()));
        r.discard();
        assert!(r.take().is_none());
        r.write(&project(), None);
        assert_eq!(r.take().unwrap().1, None, "an untitled project has no file");
    }

    #[test]
    fn a_damaged_copy_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("recovery.odp"), b"not a zip").unwrap();
        let r = Recovery::new(Some(dir.path()));
        assert!(r.take().is_none());
        assert!(!dir.path().join("recovery.odp").exists());
    }

    #[test]
    fn a_copy_from_a_newer_opendrape_is_kept_and_not_offered() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        let mut newer = project();
        newer.schema_version = opendrape_core::SCHEMA_VERSION + 1;
        let (copy, note) = (
            dir.path().join("recovery.odp"),
            dir.path().join("recovery-origin.txt"),
        );
        std::fs::write(&copy, opendrape_io::to_bytes(&newer).unwrap()).unwrap();
        std::fs::write(&note, skirt().to_str().unwrap()).unwrap();
        for _ in 0..2 {
            assert!(r.take().is_none(), "this version can't open it");
            assert!(copy.exists() && note.exists(), "but it is not deleted");
        }
        // A damaged copy in its place is still dropped.
        std::fs::write(&copy, b"not a zip").unwrap();
        assert!(r.take().is_none());
        assert!(!copy.exists() && !note.exists());
    }

    #[test]
    fn a_new_copy_forgets_the_file_of_the_one_before() {
        // Without this, work that was never saved would be offered back as if it came from the
        // file of some earlier project, and Save would write over that file.
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        r.write(&project(), Some(&skirt()));
        r.write(&project(), None);
        assert_eq!(r.take().unwrap().1, None);
        assert!(!dir.path().join("recovery-origin.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_name_that_is_not_text_is_not_remembered() {
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        r.write(&project(), Some(&skirt()));
        let odd = Path::new(std::ffi::OsStr::from_bytes(b"/work/sk\xffirt.odp"));
        r.write(&project(), Some(odd));
        let (back, from) = r.take().unwrap();
        assert_eq!(back, project(), "the work itself is still kept");
        assert_eq!(
            from, None,
            "and the earlier file is not offered in its place"
        );
    }

    #[test]
    fn the_folder_is_made_when_it_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        let r = Recovery::new(Some(&nested));
        r.write(&project(), None);
        assert_eq!(r.take().unwrap().0, project());
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_survived() {
        // OpenDrape is closing: a failure has nowhere to be shown and must not panic.
        let dir = tempfile::tempdir().unwrap();
        let not_a_folder = dir.path().join("file");
        std::fs::write(&not_a_folder, b"x").unwrap();
        let r = Recovery::new(Some(&not_a_folder));
        r.write(&project(), Some(&skirt()));
        assert!(r.take().is_none());
        r.discard();
    }

    #[test]
    fn a_damaged_copy_takes_its_file_note_with_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("recovery.odp"), b"not a zip").unwrap();
        std::fs::write(dir.path().join("recovery-origin.txt"), "/work/skirt.odp").unwrap();
        let r = Recovery::new(Some(dir.path()));
        assert!(r.take().is_none());
        assert!(!dir.path().join("recovery-origin.txt").exists());
    }

    #[test]
    fn a_file_note_that_is_empty_or_not_a_full_path_means_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        let note = dir.path().join("recovery-origin.txt");
        r.write(&project(), None);
        for bad in ["", "  \n", "relative/path.odp", "skirt.odp"] {
            std::fs::write(&note, bad).unwrap();
            let (back, from) = r.take().unwrap();
            assert_eq!(back, project(), "the work itself is still offered");
            assert_eq!(from, None, "note {bad:?}");
        }
        // A trailing newline (an editor added it) is not part of the path.
        std::fs::write(&note, format!("{}\n", skirt().display())).unwrap();
        assert_eq!(r.take().unwrap().1, Some(skirt()));
    }

    #[test]
    fn the_old_file_note_is_gone_even_when_the_new_copy_cannot_be_written() {
        // The note is deleted before the copy is written, so a write that fails part-way can't
        // leave the new project paired with the file of an earlier one.
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        r.write(&project(), Some(&skirt()));
        assert!(dir.path().join("recovery-origin.txt").exists());
        // A folder where the temporary file would go makes the save fail.
        std::fs::create_dir(dir.path().join("recovery.odp.tmp")).unwrap();
        r.write(&project(), None);
        assert!(!dir.path().join("recovery-origin.txt").exists());
        assert_eq!(
            r.take().unwrap().1,
            None,
            "an untitled restore, the safe state"
        );
    }

    #[test]
    fn discarding_carries_on_past_a_file_it_cannot_delete() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        r.write(&project(), None);
        // A folder can't be deleted as a file; the failure is logged, not fatal.
        std::fs::create_dir(dir.path().join("recovery-origin.txt")).unwrap();
        r.discard();
        assert!(!dir.path().join("recovery.odp").exists());
        assert!(dir.path().join("recovery-origin.txt").is_dir());
    }

    #[test]
    fn lives_next_to_the_graphics_settings() {
        // Only works out paths: nothing is created in the real folder.
        let gpu = crate::gpu::StateStore::default_location();
        assert_eq!(
            Recovery::default_location().dir.as_deref(),
            gpu.path().and_then(Path::parent)
        );
    }

    #[test]
    fn without_a_folder_nothing_happens() {
        let r = Recovery::new(None);
        r.write(&project(), None);
        assert!(r.take().is_none());
        r.discard();
    }
}
