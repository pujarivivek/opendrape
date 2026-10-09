//! A copy of unsaved work, written when OpenDrape is quit in a way that can't ask first (from
//! the Dock, or at logout or shutdown, where macOS ends the app without a close request) and
//! offered back the next time it starts.

use opendrape_core::Project;
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
        let dirs = directories::ProjectDirs::from("org", "OpenDrape", "OpenDrape");
        Self::new(dirs.as_ref().map(|d| d.config_local_dir()))
    }

    fn copy(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("recovery.odp"))
    }

    fn origin(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("recovery-origin.txt"))
    }

    /// Writes `project`, and the file it came from when it has one. OpenDrape is closing and
    /// can't show anything, so failures are only logged.
    pub fn write(&self, project: &Project, from: Option<&Path>) {
        let (Some(dir), Some(copy), Some(origin)) = (&self.dir, self.copy(), self.origin()) else {
            return;
        };
        if let Err(e) = std::fs::create_dir_all(dir) {
            crate::startup_log::stage(format_args!("recovery: no folder {dir:?}: {e}"));
            return;
        }
        if let Err(e) = opendrape_io::save(project, &copy) {
            crate::startup_log::stage(format_args!("recovery: could not write {copy:?}: {e}"));
            return;
        }
        let remembered = match from.and_then(Path::to_str) {
            Some(path) => std::fs::write(&origin, path),
            None => std::fs::remove_file(&origin).or(Ok(())),
        };
        if let Err(e) = remembered {
            crate::startup_log::stage(format_args!("recovery: could not note the file: {e}"));
        }
    }

    /// The waiting copy and the file it came from, if there is a copy that opens. A copy that
    /// doesn't open is deleted.
    pub fn take(&self) -> Option<(Project, Option<PathBuf>)> {
        let copy = self.copy().filter(|c| c.exists())?;
        match opendrape_io::load(&copy) {
            Ok(project) => {
                let from = self
                    .origin()
                    .and_then(|o| std::fs::read_to_string(o).ok())
                    .map(PathBuf::from);
                Some((project, from))
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
            let _ = std::fs::remove_file(file);
        }
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

    #[test]
    fn writes_and_takes_back_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        assert!(r.take().is_none());
        r.write(&project(), Some(Path::new("/work/skirt.odp")));
        let (back, from) = r.take().unwrap();
        assert_eq!(back, project());
        assert_eq!(from, Some(PathBuf::from("/work/skirt.odp")));
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
    fn a_new_copy_forgets_the_file_of_the_one_before() {
        // Without this, work that was never saved would be offered back as if it came from the
        // file of some earlier project, and Save would write over that file.
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        r.write(&project(), Some(Path::new("/work/skirt.odp")));
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
        r.write(&project(), Some(Path::new("/work/skirt.odp")));
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
        r.write(&project(), Some(Path::new("/work/skirt.odp")));
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
    fn without_a_folder_nothing_happens() {
        let r = Recovery::new(None);
        r.write(&project(), None);
        assert!(r.take().is_none());
        r.discard();
    }
}
