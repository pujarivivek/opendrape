//! OpenDrape project files: `.odp` is a zip holding one `project.json`. Loading upgrades older
//! formats, refuses newer ones and rejects invalid data, so a bad file never replaces a
//! student's work.

use opendrape_core::{ModelError, Project, SCHEMA_VERSION};
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const EXTENSION: &str = "odp";
const ENTRY: &str = "project.json";
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum OdpError {
    Io(std::io::Error),
    NotAProject,
    Corrupt(String),
    NewerVersion { found: u64, supported: u32 },
    Invalid(ModelError),
    TooLarge,
}

impl std::fmt::Display for OdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read or write the file: {e}"),
            Self::NotAProject => write!(f, "not an OpenDrape project file"),
            Self::Corrupt(why) => write!(f, "damaged project file ({why})"),
            Self::NewerVersion { found, supported } => {
                write!(
                    f,
                    "made by a newer OpenDrape (format {found}; this version reads up to {supported})"
                )
            }
            Self::Invalid(e) => write!(f, "invalid project data ({e})"),
            Self::TooLarge => write!(f, "project file too large"),
        }
    }
}

impl std::error::Error for OdpError {}

impl From<std::io::Error> for OdpError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

fn zip_err(e: zip::result::ZipError) -> OdpError {
    match e {
        zip::result::ZipError::Io(e) => OdpError::Io(e),
        other => OdpError::Corrupt(other.to_string()),
    }
}

fn write_to<W: Write + Seek>(project: &Project, w: W) -> Result<(), OdpError> {
    let mut zip = ZipWriter::new(w);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    zip.start_file(ENTRY, options).map_err(zip_err)?;
    serde_json::to_writer_pretty(&mut zip, project)
        .map_err(|e| OdpError::Corrupt(e.to_string()))?;
    zip.finish().map_err(zip_err)?;
    Ok(())
}

fn read_from<R: Read + Seek>(r: R) -> Result<Project, OdpError> {
    let mut zip = ZipArchive::new(r).map_err(|_| OdpError::NotAProject)?;
    let mut entry = zip.by_name(ENTRY).map_err(|_| OdpError::NotAProject)?;
    if entry.size() > MAX_JSON_BYTES {
        return Err(OdpError::TooLarge);
    }
    let mut text = String::new();
    entry
        .by_ref()
        .take(MAX_JSON_BYTES)
        .read_to_string(&mut text)
        .map_err(|e| OdpError::Corrupt(e.to_string()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    let value = migrate(value)?;
    let project: Project =
        serde_json::from_value(value).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    project.check().map_err(OdpError::Invalid)?;
    Ok(project)
}

/// Upgrades older project JSON to [`SCHEMA_VERSION`] one version at a time. Version 1 is the
/// first format, so there are no steps yet: add `found = 1 => { …; found = 2 }` arms here.
fn migrate(value: serde_json::Value) -> Result<serde_json::Value, OdpError> {
    let found = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(OdpError::NotAProject)?;
    if found > u64::from(SCHEMA_VERSION) {
        return Err(OdpError::NewerVersion {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    if found == 0 {
        return Err(OdpError::Corrupt("format version 0".into()));
    }
    Ok(value)
}

pub fn to_bytes(project: &Project) -> Result<Vec<u8>, OdpError> {
    let mut out = Cursor::new(Vec::new());
    write_to(project, &mut out)?;
    Ok(out.into_inner())
}

pub fn from_bytes(bytes: &[u8]) -> Result<Project, OdpError> {
    read_from(Cursor::new(bytes))
}

/// Saves atomically: writes `<path>.odp.tmp`, then renames it over `path`.
pub fn save(project: &Project, path: &Path) -> Result<(), OdpError> {
    let tmp = path.with_extension(format!("{EXTENSION}.tmp"));
    let result = (|| {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        write_to(project, &mut w)?;
        w.flush()?;
        drop(w);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn load(path: &Path) -> Result<Project, OdpError> {
    read_from(std::io::BufReader::new(std::fs::File::open(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, PieceId, Point2};

    fn sample() -> Project {
        let mut p = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 350.0, 550.0);
        front.set_curved(2, true);
        p.add_piece(front);
        p
    }

    fn zip_with(entry: &str, json: &str) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(entry, SimpleFileOptions::default()).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn round_trips_and_is_reproducible() {
        let bytes = to_bytes(&sample()).unwrap();
        assert_eq!(from_bytes(&bytes).unwrap(), sample());
        assert_eq!(
            bytes,
            to_bytes(&sample()).unwrap(),
            "same project, same bytes"
        );
    }

    #[test]
    fn refuses_a_newer_format() {
        let json = r#"{"schema_version": 99, "pieces": []}"#;
        assert!(matches!(
            from_bytes(&zip_with("project.json", json)),
            Err(OdpError::NewerVersion { found: 99, .. })
        ));
    }

    #[test]
    fn refuses_files_that_are_not_projects() {
        assert!(matches!(from_bytes(b"hello"), Err(OdpError::NotAProject)));
        assert!(matches!(
            from_bytes(&zip_with("other.txt", "{}")),
            Err(OdpError::NotAProject)
        ));
        assert!(matches!(
            from_bytes(&zip_with("project.json", "{not json")),
            Err(OdpError::Corrupt(_))
        ));
        assert!(matches!(
            from_bytes(&zip_with("project.json", "{}")),
            Err(OdpError::NotAProject)
        ));
    }

    #[test]
    fn refuses_invalid_geometry() {
        let mut bad = sample();
        bad.pieces[0].vertices.truncate(2);
        bad.pieces[0].edges.truncate(2);
        let json = serde_json::to_string(&bad).unwrap();
        assert!(matches!(
            from_bytes(&zip_with("project.json", &json)),
            Err(OdpError::Invalid(_))
        ));
    }

    #[test]
    fn saves_atomically_and_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skirt.odp");
        save(&sample(), &path).unwrap();
        assert_eq!(load(&path).unwrap(), sample());
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            vec![std::ffi::OsString::from("skirt.odp")],
            "no temp file left behind"
        );
        assert!(matches!(
            load(&dir.path().join("missing.odp")),
            Err(OdpError::Io(_))
        ));
    }
}
