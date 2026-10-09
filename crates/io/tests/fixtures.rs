//! Frozen project files, one folder per format version (see `fixtures/README.md`). A future
//! build must still open every one of them and see exactly the project written down here.
//! The expected projects are spelled out field by field and never go through the save code,
//! so a change to how projects are saved cannot quietly change what these files mean.

use opendrape_core::{
    Edge, EdgeProps, InternalLine, LineKind, Notch, NotchStyle, Piece, PieceId, Point2, Project,
    Twin, Units, Vertex, VertexKind,
};
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
        allowance: 10.0,
        edge_props: vec![EdgeProps::default(); 4],
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None,
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
        allowance: 10.0,
        edge_props: vec![EdgeProps::default(); 3],
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None,
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
    assert_eq!(loaded.schema_version, 2, "upgraded on load");
    assert_eq!(loaded.next_piece_name("Piece"), "Piece 7");
}

#[test]
fn format_v2_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v2/project.json")))
        .expect("the frozen v2 project opens");
    let props = |allowance: Option<f64>, hem: bool| EdgeProps { allowance, hem };
    let front = Piece {
        id: PieceId(1),
        name: "Skirt front".into(),
        vertices: vec![
            corner(0.0, 0.0),
            corner(250.0, 0.0),
            corner(200.0, 600.0),
            corner(0.0, 600.0),
        ],
        edges: vec![
            Edge::Line,
            Edge::Line,
            Edge::Curve {
                c1: at(130.0, 610.0),
                c2: at(60.0, 605.0),
            },
            Edge::Line,
        ],
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: vec![
            props(None, true),
            props(Some(15.0), false),
            props(None, false),
            props(None, false),
        ],
        notches: vec![
            Notch {
                edge: 1,
                distance: 180.0,
                marks: 1,
                style: NotchStyle::Slit,
            },
            Notch {
                edge: 2,
                distance: 40.0,
                marks: 2,
                style: NotchStyle::V,
            },
        ],
        lines: vec![InternalLine {
            vertices: vec![corner(30.0, 300.0), corner(120.0, 300.0)],
            edges: vec![Edge::Line],
            closed: false,
            kind: LineKind::Marking,
        }],
        fold: Some(3),
        twin: None,
    };
    let back = Piece {
        id: PieceId(2),
        name: "Back left".into(),
        vertices: vec![
            corner(400.0, 0.0),
            corner(600.0, 0.0),
            corner(580.0, 600.0),
            corner(400.0, 600.0),
        ],
        edges: vec![Edge::Line; 4],
        grain_deg: 90.0,
        allowance: 12.0,
        edge_props: vec![
            props(None, true),
            props(None, false),
            props(None, false),
            props(None, false),
        ],
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None, // set below, once the ids are taken
    };
    let mut expected = Project::new(); // units default to cm
    expected.add_piece(front);
    expected.add_piece(back);
    // The twin has id 3 and the file's counter stands at 4. Take id 3 with a throwaway piece
    // (the only way to move the counter from outside the model), then give the back its twin.
    let spare = expected.add_piece(Piece::polygon(
        PieceId(0),
        "x",
        &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)],
    ));
    expected.remove_piece(spare);
    expected.pieces[1].twin = Some(Twin {
        id: PieceId(3),
        name: "Back right".into(),
        offset: at(1300.0, 0.0),
    });
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.name_of(PieceId(3)), Some("Back right"));
    assert_eq!(loaded.next_piece_name("Piece"), "Piece 4");
}

#[test]
fn refuses_invalid_v2_details() {
    let good = include_str!("fixtures/v2/project.json");
    for (from, to) in [
        (r#""fold": 3"#, r#""fold": 2"#), // a curved edge
        (
            r#""edge": 1, "distance": 180.0"#,
            r#""edge": 9, "distance": 180.0"#, // no such edge
        ),
        (
            r#""twin": null"#,
            r#""twin": { "id": 5, "name": "x", "offset": { "x": 0, "y": 0 } }"#, // folded and paired
        ),
        (r#""allowance": 12.0"#, r#""allowance": 500.0"#),
    ] {
        assert!(good.contains(from), "{from}");
        let bad = good.replacen(from, to, 1);
        assert!(
            matches!(
                opendrape_io::from_bytes(&odp(&bad)),
                Err(opendrape_io::OdpError::Invalid(_))
            ),
            "{to}"
        );
    }
}
