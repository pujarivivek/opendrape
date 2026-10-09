//! Frozen project files, one folder per format version (see `fixtures/README.md`). A future
//! build must still open every one of them and see exactly the project written down here.
//! The expected projects are spelled out field by field and never go through the save code,
//! so a change to how projects are saved cannot quietly change what these files mean.

use opendrape_core::{Edge, Piece, PieceId, Point2, Project, Units, Vertex, VertexKind};
use std::io::{Cursor, Write};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// The text of a fixture, packed as a `.odp` archive.
fn odp(json: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("project.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(json.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

fn at(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

fn corner(x: f64, y: f64) -> Vertex {
    Vertex {
        pos: at(x, y),
        kind: VertexKind::Corner,
    }
}

#[test]
fn format_v1_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v1/project.json")))
        .expect("the frozen v1 project opens");

    let front = Piece {
        id: PieceId(1),
        name: "Front panel".into(),
        vertices: vec![
            corner(0.0, 0.0),
            corner(500.0, 0.0),
            Vertex {
                pos: at(500.0, 800.0),
                kind: VertexKind::Smooth,
            },
            corner(0.0, 800.0),
        ],
        edges: vec![
            Edge::Line,
            Edge::Curve {
                c1: at(560.0, 250.0),
                c2: at(560.0, 550.0),
            },
            Edge::Line,
            Edge::Line,
        ],
        grain_deg: 90.0,
    };
    let pocket = Piece {
        id: PieceId(2),
        name: "Pocket".into(),
        vertices: vec![
            corner(100.0, 100.0),
            corner(300.0, 100.0),
            corner(200.0, 250.0),
        ],
        edges: vec![Edge::Line, Edge::Line, Edge::Line],
        grain_deg: 45.5,
    };

    // The file says the id counter stands at 7: four more pieces were drawn and deleted
    // after these two. The only way to set a counter from outside the model is to do that.
    let mut expected = Project::new();
    expected.units = Units::Inch;
    expected.add_piece(front);
    expected.add_piece(pocket);
    let spare: Vec<PieceId> = (0..4)
        .map(|_| {
            expected.add_piece(Piece::polygon(
                PieceId(0),
                "x",
                &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)],
            ))
        })
        .collect();
    for id in spare {
        expected.remove_piece(id);
    }

    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.next_piece_name("Piece"), "Piece 7");
}
