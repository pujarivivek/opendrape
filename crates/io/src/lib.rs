//! OpenDrape project files: `.odp` is a zip holding one `project.json`. Loading upgrades older
//! formats, refuses newer ones and rejects invalid data, so a bad file never replaces a
//! student's work.

use opendrape_core::{ModelError, Project, SCHEMA_VERSION};
use serde::Deserialize;
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const EXTENSION: &str = "odp";
const ENTRY: &str = "project.json";
/// Largest `project.json` we read. Bigger entries are refused before they are parsed.
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;

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
                    "made by a newer OpenDrape (format {found}; this version reads up to {supported}); update OpenDrape to open it"
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

/// Writing: a real I/O failure stays `Io`; anything else means the archive could not be built.
fn zip_err(e: zip::result::ZipError) -> OdpError {
    match e {
        zip::result::ZipError::Io(e) => OdpError::Io(e),
        other => OdpError::Corrupt(other.to_string()),
    }
}

/// Reading: a real I/O failure stays `Io`; anything else means this is not a project archive.
fn read_zip_err(e: zip::result::ZipError) -> OdpError {
    match e {
        zip::result::ZipError::Io(e) => OdpError::Io(e),
        _ => OdpError::NotAProject,
    }
}

fn write_to<W: Write + Seek>(project: &Project, w: W) -> Result<(), OdpError> {
    let mut zip = ZipWriter::new(w);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    zip.start_file(ENTRY, options).map_err(zip_err)?;
    serde_json::to_writer_pretty(&mut zip, project).map_err(|e| {
        if e.is_io() {
            OdpError::Io(std::io::Error::from(e))
        } else {
            OdpError::Corrupt(e.to_string())
        }
    })?;
    zip.finish().map_err(zip_err)?;
    Ok(())
}

fn read_from<R: Read + Seek>(r: R) -> Result<Project, OdpError> {
    let mut zip = ZipArchive::new(r).map_err(read_zip_err)?;
    let mut entry = zip.by_name(ENTRY).map_err(read_zip_err)?;
    if entry.size() > MAX_JSON_BYTES {
        return Err(OdpError::TooLarge);
    }
    // Read one byte past the cap, so an entry whose header lies about its size is still caught.
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .by_ref()
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| OdpError::Corrupt(e.to_string()))?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(OdpError::TooLarge);
    }
    let text = std::str::from_utf8(&bytes).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    let found = check_version(text)?;
    let mut project: Project = if found <= 3 {
        parse_older(text)?
    } else {
        // The current format is read straight into a `Project`: no tree of the whole file
        // is built first (it takes several times the file's size in memory).
        serde_json::from_str(text).map_err(|e| OdpError::Corrupt(e.to_string()))?
    };
    if found == 1 {
        upgrade_from_v1(&mut project);
    }
    // Version 2 (M2b) had no seams or 3D placements; serde's defaults give an older file none.
    // Whatever version it was, it is the current one now.
    project.schema_version = SCHEMA_VERSION;
    project.check().map_err(OdpError::Invalid)?;
    Ok(project)
}

/// A file of version 1 to 3, read as a `serde_json::Value` first so that the seam sides of
/// version 3 can be rewritten ([`upgrade_sides_from_v3`]); the fields older versions lack take
/// their defaults.
fn parse_older(text: &str) -> Result<Project, OdpError> {
    #[cfg(test)]
    tests::VALUE_PARSES.with(|n| n.set(n.get() + 1));
    let mut document: serde_json::Value =
        serde_json::from_str(text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    upgrade_sides_from_v3(&mut document);
    serde_json::from_value(document).map_err(|e| OdpError::Corrupt(e.to_string()))
}

/// Version 3 (M4a) stored a seam side as whole edges: `first_edge`, `edges` and `forward`. From
/// version 4 a side runs between two points of the outline, so each becomes the free side from
/// the start of its first edge to the end of its last (from the last's end to the first's start
/// when it runs the other way). The edge count comes from the piece the side is on (a twin has
/// its piece's). A side that names edges its piece doesn't have, or none, ends on an edge past
/// the last, so the check that follows refuses it as before.
fn upgrade_sides_from_v3(document: &mut serde_json::Value) {
    use serde_json::{Value, json};
    let mut edge_counts = std::collections::BTreeMap::new();
    for piece in document["pieces"].as_array().into_iter().flatten() {
        let n = piece["vertices"].as_array().map_or(0, Vec::len) as u64;
        edge_counts.extend(piece["id"].as_u64().map(|id| (id, n)));
        edge_counts.extend(piece["twin"]["id"].as_u64().map(|id| (id, n)));
    }
    let Some(seams) = document.get_mut("seams").and_then(Value::as_array_mut) else {
        return;
    };
    for seam in seams {
        for key in ["a", "b"] {
            let Some(side) = seam.get_mut(key).and_then(Value::as_object_mut) else {
                continue;
            };
            let (Some(first), Some(edges), Some(forward)) = (
                side.get("first_edge").and_then(Value::as_u64),
                side.get("edges").and_then(Value::as_u64),
                side.get("forward").and_then(Value::as_bool),
            ) else {
                continue;
            };
            let n = side
                .get("shape")
                .and_then(Value::as_u64)
                .and_then(|id| edge_counts.get(&id).copied())
                .unwrap_or(0);
            // `first < n` and `edges <= n` keep the sum small (n is a count of vertices in the
            // file). Anything else is put on an edge the piece lacks, however large the numbers.
            let last = if first < n && (1..=n).contains(&edges) {
                (first + edges - 1) % n
            } else {
                n.max(first.saturating_add(1))
            };
            let (start, end) = (
                json!({"edge": first, "t": 0.0}),
                json!({"edge": last, "t": 1.0}),
            );
            let (from, to) = if forward { (start, end) } else { (end, start) };
            side.remove("first_edge");
            side.remove("edges");
            side.insert("from".into(), from);
            side.insert("to".into(), to);
        }
    }
}

/// Version 1 (M2a) had no seam allowances, notches, internal lines, folds or twins. Serde's
/// field defaults already give a v1 file every new field except one set of edge properties per
/// edge, which needs the edge count; add those.
fn upgrade_from_v1(project: &mut Project) {
    for piece in &mut project.pieces {
        piece.edge_props = vec![opendrape_core::EdgeProps::default(); piece.edges.len()];
    }
}

/// Reads only `schema_version` (other fields are skipped without building anything), so a file
/// from a newer format is reported as such even when its contents have changed shape. Returns
/// the version found.
///
/// Versions 1 to 3 are read as a `serde_json::Value` first ([`parse_older`]); the fields they lack
/// take their defaults (see [`upgrade_from_v1`]).
fn check_version(text: &str) -> Result<u64, OdpError> {
    #[derive(Deserialize)]
    struct Version {
        schema_version: Option<u64>,
    }
    let version: Version =
        serde_json::from_str(text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    let found = version.schema_version.ok_or(OdpError::NotAProject)?;
    if found > u64::from(SCHEMA_VERSION) {
        return Err(OdpError::NewerVersion {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    if found == 0 {
        return Err(OdpError::Corrupt("format version 0".into()));
    }
    Ok(found)
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
        // Force the bytes to disk before the rename. Otherwise a power cut can leave the new name
        // pointing at an empty or partial file, in place of the last good save.
        w.get_ref().sync_all()?;
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

    thread_local! {
        /// How many files this test thread has read as a `serde_json::Value` (see
        /// [`parse_older`]).
        pub(super) static VALUE_PARSES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }

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

    /// How many files `f` reads as a `serde_json::Value`.
    fn value_parses(f: impl FnOnce()) -> u32 {
        let before = VALUE_PARSES.with(std::cell::Cell::get);
        f();
        VALUE_PARSES.with(std::cell::Cell::get) - before
    }

    #[test]
    fn a_current_file_is_read_straight_into_the_project_and_an_older_one_through_a_value() {
        // A big hostile file would build a tree several times its own size on the thread that
        // opens it: only the formats that need rewriting go through one.
        let current = to_bytes(&sample()).unwrap();
        assert_eq!(
            value_parses(|| assert_eq!(from_bytes(&current).unwrap(), sample())),
            0,
            "version 4 is typed at once"
        );
        for (version, json) in [
            (1, include_str!("../tests/fixtures/v1/project.json")),
            (2, include_str!("../tests/fixtures/v2/project.json")),
            (3, include_str!("../tests/fixtures/v3/project.json")),
        ] {
            let packed = zip_with("project.json", json);
            assert_eq!(
                value_parses(|| {
                    from_bytes(&packed).unwrap();
                }),
                1,
                "version {version} still loads, through the upgrade"
            );
        }
        // And what the typed path refuses is still refused as damaged, not as anything else.
        let bad = zip_with("project.json", r#"{"schema_version": 4, "pieces": 7}"#);
        assert!(matches!(from_bytes(&bad), Err(OdpError::Corrupt(_))));
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
    fn refuses_a_piece_with_too_many_points() {
        // A file well under the size cap can still hold a piece that would take minutes to draw.
        let n = 3_000;
        let corners: Vec<Point2> = (0..n)
            .map(|k| {
                let a = f64::from(k) / f64::from(n) * std::f64::consts::TAU;
                Point2::new(500.0 * a.cos(), 500.0 * a.sin())
            })
            .collect();
        let mut bad = Project::new();
        bad.add_piece(Piece::polygon(PieceId(0), "Ring", &corners));
        let json = serde_json::to_string(&bad).unwrap();
        assert!(json.len() < 1024 * 1024, "far below the size cap");
        let Err(err) = from_bytes(&zip_with("project.json", &json)) else {
            panic!("a 3000-point piece was accepted");
        };
        assert!(matches!(err, OdpError::Invalid(_)), "{err}");
        assert!(err.to_string().contains("too many points"), "{err}");
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

    #[test]
    fn refuses_an_oversized_entry() {
        let json = format!(
            r#"{{"schema_version":1,"pad":"{}"}}"#,
            "a".repeat(MAX_JSON_BYTES as usize)
        );
        assert!(matches!(
            from_bytes(&zip_with("project.json", &json)),
            Err(OdpError::TooLarge)
        ));
    }

    #[test]
    fn refuses_an_entry_that_lies_about_its_size() {
        // A stored entry whose real data is over the cap, with its declared size patched to 100
        // bytes in the local header (offset 22) and the central directory (24 bytes in).
        let json = format!(
            r#"{{"schema_version":1,"pad":"{}"}}"#,
            "a".repeat(MAX_JSON_BYTES as usize)
        );
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("project.json", stored).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        let mut bytes = zip.finish().unwrap().into_inner();
        let central = bytes.windows(4).rposition(|w| w == b"PK\x01\x02").unwrap();
        bytes[22..26].copy_from_slice(&100u32.to_le_bytes());
        bytes[central + 24..central + 28].copy_from_slice(&100u32.to_le_bytes());
        assert!(matches!(from_bytes(&bytes), Err(OdpError::TooLarge)));
    }

    /// A disk that fills up after about 1 KiB.
    struct FullDisk(Cursor<Vec<u8>>);

    impl Write for FullDisk {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.0.get_ref().len() >= 1024 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::StorageFull,
                    "disk full",
                ));
            }
            self.0.write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }

    impl Seek for FullDisk {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.0.seek(pos)
        }
    }

    #[test]
    fn reports_a_full_disk_as_an_io_error() {
        let mut big = Project::new();
        for i in 0..5_000 {
            big.add_piece(Piece::rectangle(
                PieceId(0),
                format!("Piece {i}"),
                Point2::new(i as f64, 0.0),
                10.0,
                20.0,
            ));
        }
        assert!(matches!(
            write_to(&big, FullDisk(Cursor::new(Vec::new()))),
            Err(OdpError::Io(_))
        ));
    }

    /// A disk that fails on every access.
    struct BrokenDisk;

    impl Read for BrokenDisk {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk failed"))
        }
    }

    impl Seek for BrokenDisk {
        fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
            Err(std::io::Error::other("disk failed"))
        }
    }

    #[test]
    fn reports_a_failed_read_as_an_io_error() {
        assert!(matches!(read_from(BrokenDisk), Err(OdpError::Io(_))));
    }
}
