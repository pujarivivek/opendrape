//! Frozen project files, one folder per format version (see `fixtures/README.md`). A future
//! build must still open every one of them and see exactly the project written down here.
//! The expected projects are spelled out field by field and never go through the save code,
//! so a change to how projects are saved cannot quietly change what these files mean.

use opendrape_core::{
    Edge, EdgeProps, Half, InternalLine, LineKind, Notch, NotchStyle, OutlinePos, Piece, PieceId,
    Pin, Placement, Point2, Project, Seam, SeamId, SeamSide, Twin, Units, Vertex, VertexKind,
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
        placement: None,
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
        placement: None,
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
    assert_eq!(loaded.schema_version, 4, "upgraded on load");
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
        placement: None,
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
        placement: None,
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
        placement: None,
    });
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 4, "upgraded on load");
    assert!(loaded.seams.is_empty());
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

#[test]
fn format_v3_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v3/project.json")))
        .expect("the frozen v3 project opens");
    let lines = || vec![Edge::Line; 4];
    let props = |hem: bool| {
        let mut p = vec![EdgeProps::default(); 4];
        p[0].hem = hem;
        p
    };
    let front = Piece {
        id: PieceId(1),
        name: "Skirt front".into(),
        vertices: vec![
            corner(0.0, 0.0),
            corner(300.0, 0.0),
            corner(177.5, 550.0),
            corner(0.0, 550.0),
        ],
        edges: lines(),
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true),
        notches: vec![],
        lines: vec![],
        fold: Some(3),
        twin: None,
        placement: Some(Placement {
            position: [0.0, 0.755, 0.214],
            rotation: [0.0, 0.0, 0.0, 1.0],
            curve: Some(0.214),
        }),
    };
    let back = Piece {
        id: PieceId(2),
        name: "Back left".into(),
        vertices: vec![
            corner(400.0, 0.0),
            corner(700.0, 0.0),
            corner(700.0, 550.0),
            corner(522.5, 550.0),
        ],
        edges: lines(),
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true),
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None, // set below, once the ids are taken
        placement: None,
    };
    let pocket = Piece {
        id: PieceId(4),
        name: "Pocket".into(),
        vertices: vec![
            corner(0.0, 700.0),
            corner(150.0, 700.0),
            corner(150.0, 850.0),
            corner(0.0, 850.0),
        ],
        edges: lines(),
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(false),
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None,
        placement: None,
    };
    let mut expected = Project::new();
    expected.units = Units::Inch;
    expected.add_piece(front);
    expected.add_piece(back);
    // The twin has id 3: take it with a throwaway piece (the only way to move the counter
    // from outside the model), then give the back its twin.
    let spare = expected.add_piece(Piece::polygon(
        PieceId(0),
        "x",
        &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)],
    ));
    expected.remove_piece(spare);
    expected.add_piece(pocket);
    expected.pieces[1].twin = Some(Twin {
        id: PieceId(3),
        name: "Back right".into(),
        offset: at(1500.0, 0.0),
        placement: Some(Placement {
            position: [-0.1, 0.755, -0.2],
            rotation: [0.0, 1.0, 0.0, 0.0],
            curve: None,
        }),
    });
    // Version 3 sewed whole edges: each side is now the free side from its first edge's start
    // to its last edge's end (the other way round when it ran backwards).
    let side = |shape: u32, half: Half, first: usize, last: usize, forward: bool| {
        let (start, end) = (OutlinePos::new(first, 0.0), OutlinePos::new(last, 1.0));
        let (from, to) = if forward { (start, end) } else { (end, start) };
        SeamSide {
            shape: PieceId(shape),
            half,
            from,
            to,
            forward,
        }
    };
    expected.seams = vec![
        Seam {
            id: SeamId(1),
            a: side(1, Half::Drawn, 1, 1, true),
            b: side(2, Half::Drawn, 3, 3, false),
        },
        Seam {
            id: SeamId(2),
            a: side(2, Half::Drawn, 1, 1, true),
            b: side(3, Half::Drawn, 1, 1, true),
        },
        Seam {
            id: SeamId(5),
            a: side(4, Half::Drawn, 3, 0, true),
            b: side(1, Half::Pale, 2, 2, false),
        },
    ];
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 4, "upgraded on load");
    assert_eq!(loaded.next_piece_name("Piece"), "Piece 5");
    // The first seam has a mirror image (front's pale half to the back's twin); the centre
    // back is its own; the pocket has none.
    assert_eq!(loaded.all_seams().len(), 4);
}

#[test]
fn refuses_invalid_v3_details() {
    let good = include_str!("fixtures/v3/project.json");
    for (from, to) in [
        (r#""curve": 0.214"#, r#""curve": 0.01"#), // too tight a curve
        (
            r#""rotation": [0.0, 1.0, 0.0, 0.0]"#,
            r#""rotation": [0.0, 2.0, 0.0, 0.0]"#,
        ), // not unit length
        (
            r#""first_edge": 3, "edges": 1, "forward": false"#,
            r#""first_edge": 3, "edges": 9, "forward": false"#,
        ), // more edges than the outline
        (
            r#""shape": 4, "half": "drawn""#,
            r#""shape": 4, "half": "pale""#,
        ), // the pocket isn't folded
        (
            r#""shape": 1, "half": "pale", "first_edge": 2"#,
            r#""shape": 1, "half": "pale", "first_edge": 3"#,
        ), // the fold edge
        (
            r#""shape": 3, "half": "drawn", "first_edge": 1"#,
            r#""shape": 3, "half": "drawn", "first_edge": 3"#,
        ), // the twin's edge 3 is sewn by seam 1's mirror
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

#[test]
fn format_v3_with_a_whole_edge_seam_on_a_sub_millimetre_edge_opens_without_that_seam() {
    // M4a had no minimum length for a seam, so a student could sew a 0.5 mm edge with W. The
    // new rules refuse such a side; the file must still open, with only that seam dropped.
    let good = include_str!("fixtures/v3/project.json");
    // The pocket's top edge (edge 2) made 0.5 mm long.
    let (from_right, from_left) = (r#""x": 150.0, "y": 850.0"#, r#""x": 0.0, "y": 850.0"#);
    assert!(good.contains(from_right) && good.contains(from_left));
    let short_top = good
        .replacen(from_right, r#""x": 75.25, "y": 850.0"#, 1)
        .replacen(from_left, r#""x": 74.75, "y": 850.0"#, 1);
    let kept = r#"{ "id": 1, "a": { "shape": 1, "half": "drawn", "first_edge": 1, "edges": 1, "forward": true }, "b": { "shape": 2, "half": "drawn", "first_edge": 3, "edges": 1, "forward": false } },
    { "id": 2, "a": { "shape": 2, "half": "drawn", "first_edge": 1, "edges": 1, "forward": true }, "b": { "shape": 3, "half": "drawn", "first_edge": 1, "edges": 1, "forward": true } },
    { "id": 5, "a": { "shape": 4, "half": "drawn", "first_edge": 3, "edges": 2, "forward": true }, "b": { "shape": 1, "half": "pale", "first_edge": 2, "edges": 1, "forward": false } }"#;
    let probe = r#", { "id": 6, "a": { "shape": 4, "half": "drawn", "first_edge": 2, "edges": 1, "forward": true }, "b": { "shape": 2, "half": "drawn", "first_edge": 0, "edges": 1, "forward": true } }"#;
    let with = |seams: &str| {
        let (head, rest) = short_top.split_once(r#""seams": ["#).unwrap();
        let (_, tail) = rest.split_once(r#""next_piece_id""#).unwrap();
        format!(r#"{head}"seams": [{seams}], "next_piece_id"{tail}"#)
    };

    let without_the_probe = opendrape_io::from_bytes(&odp(&with(kept)))
        .expect("the file without the short seam opens, so the pocket is a good piece");
    assert_eq!(without_the_probe.seams.len(), 3);
    let opened = opendrape_io::from_bytes(&odp(&with(&format!("{kept}{probe}"))))
        .expect("the file with the short seam opens too");
    let ids: Vec<u32> = opened.seams.iter().map(|s| s.id.0).collect();
    assert_eq!(ids, [1, 2, 5], "only the 0.5 mm seam is gone");
    assert_eq!(opened, without_the_probe, "and nothing else changed");
    assert_eq!(opened.schema_version, 4);

    // A seam that is wrong in another way is still refused, short seam or not: the repair is
    // for the new length rule only.
    let also_bad = format!(
        "{kept}{probe}, {}",
        r#"{ "id": 7, "a": { "shape": 4, "half": "pale", "first_edge": 0, "edges": 1, "forward": true }, "b": { "shape": 2, "half": "drawn", "first_edge": 2, "edges": 1, "forward": true } }"#
    );
    assert!(matches!(
        opendrape_io::from_bytes(&odp(&with(&also_bad))),
        Err(opendrape_io::OdpError::Invalid(_))
    ));
}

/// The frozen v3 project with its seams replaced by `seams` (the inside of the JSON array).
fn v3_with_seams(seams: &str) -> String {
    let good = include_str!("fixtures/v3/project.json");
    let (head, rest) = good
        .split_once(r#""seams": ["#)
        .expect("the fixture has seams");
    let (_, tail) = rest
        .split_once(r#""next_piece_id""#)
        .expect("the fixture counts its pieces");
    format!(r#"{head}"seams": [{seams}], "next_piece_id"{tail}"#)
}

/// A version 3 seam side as the file spells it.
fn v3_side(shape: &str, half: &str, first: &str, edges: &str, forward: bool) -> String {
    format!(
        r#"{{ "shape": {shape}, "half": "{half}", "first_edge": {first}, "edges": {edges}, "forward": {forward} }}"#
    )
}

#[test]
fn format_v3_sides_running_backward_over_several_edges_upgrade_from_their_last_edge() {
    let p = OutlinePos::new;
    // The pocket (4 edges): edges 3 and 0 backward, wrapping. The twin of the back (also 4
    // edges): edges 3 and 0 forward, wrapping, sewn to the pocket's edges 1 and 2.
    let json = v3_with_seams(&format!(
        r#"{{ "id": 1, "a": {}, "b": {} }}, {{ "id": 2, "a": {}, "b": {} }}"#,
        v3_side("4", "drawn", "3", "2", false),
        v3_side("1", "drawn", "0", "2", true),
        v3_side("3", "drawn", "3", "2", true),
        v3_side("4", "drawn", "1", "2", true),
    ));
    let loaded = opendrape_io::from_bytes(&odp(&json)).expect("opens");
    let (first, second) = (&loaded.seams[0], &loaded.seams[1]);
    // Running backward it starts at the end of the last edge, and ends at the start of the first.
    assert_eq!(
        (first.a.from, first.a.to, first.a.forward),
        (p(0, 1.0), p(3, 0.0), false)
    );
    assert_eq!(
        (first.b.from, first.b.to, first.b.forward),
        (p(0, 0.0), p(1, 1.0), true)
    );
    // The twin has its piece's four edges: the wrap works there too.
    assert_eq!(
        (second.a.shape, second.a.from, second.a.to, second.a.forward),
        (PieceId(3), p(3, 0.0), p(0, 1.0), true)
    );
    assert_eq!(
        (second.b.from, second.b.to),
        (p(1, 0.0), p(2, 1.0)),
        "no wrap, forward"
    );
    // The same stretch of outline the old whole-edge side covered.
    assert_eq!(first.a.spans(4).len(), 2);
    assert!(first.a.covers(4, 3) && first.a.covers(4, 0));
}

#[test]
fn format_v3_sides_covering_every_edge_upgrade_to_the_whole_outline() {
    let p = OutlinePos::new;
    for (first, forward, from, to) in [
        ("1", true, p(1, 0.0), p(0, 1.0)),
        ("2", false, p(1, 1.0), p(2, 0.0)),
    ] {
        let json = v3_with_seams(&format!(
            r#"{{ "id": 1, "a": {}, "b": {} }}"#,
            v3_side("4", "drawn", first, "4", forward),
            v3_side("2", "drawn", "0", "1", true),
        ));
        let loaded = opendrape_io::from_bytes(&odp(&json)).expect("opens");
        let a = loaded.seams[0].a;
        assert_eq!((a.from, a.to, a.forward), (from, to, forward));
        assert_eq!(a.spans(4).len(), 4, "every edge, once");
    }
}

#[test]
fn format_v3_sides_naming_edges_the_piece_lacks_are_refused() {
    let seam = |a: String| {
        v3_with_seams(&format!(
            r#"{{ "id": 7, "a": {a}, "b": {} }}"#,
            v3_side("2", "drawn", "0", "1", true)
        ))
    };
    let u64_max = u64::MAX.to_string();
    for (a, why) in [
        (v3_side("4", "drawn", "3", "0", true), "no edges"),
        (
            v3_side("4", "drawn", "3", "9", true),
            "more edges than there are",
        ),
        (
            v3_side("4", "drawn", "4", "1", true),
            "the first edge is past the last",
        ),
        (
            v3_side("4", "drawn", &u64_max, "1", true),
            "first edge as large as can be",
        ),
        (
            v3_side("4", "drawn", &u64_max, "1", false),
            "the same, running backward",
        ),
        (
            v3_side("4", "drawn", "3", &u64_max, true),
            "as many edges as can be",
        ),
        (
            v3_side("99", "drawn", &u64_max, "1", true),
            "a shape that isn't there",
        ),
    ] {
        // A hostile number must be refused, never overflow (tests run with overflow checks).
        let result = opendrape_io::from_bytes(&odp(&seam(a)));
        assert!(
            matches!(result, Err(opendrape_io::OdpError::Invalid(_))),
            "{why}: {result:?}"
        );
    }
}

#[test]
fn format_v4_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v4/project.json")))
        .expect("the frozen v4 project opens");
    let props = |hem: bool, n: usize| {
        let mut p = vec![EdgeProps::default(); n];
        p[0].hem = hem;
        p
    };
    let front = Piece {
        id: PieceId(1),
        name: "Front".into(),
        vertices: vec![
            corner(0.0, 0.0),
            corner(200.0, 0.0),
            corner(200.0, 350.0),
            corner(150.0, 450.0),
            corner(0.0, 450.0),
        ],
        edges: vec![
            Edge::Line,
            Edge::Line,
            Edge::Curve {
                c1: at(190.0, 400.0),
                c2: at(170.0, 440.0),
            },
            Edge::Line,
            Edge::Line,
        ],
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true, 5),
        notches: vec![Notch::new(2, 30.0)],
        lines: vec![],
        fold: Some(4),
        twin: None,
        placement: Some(Placement {
            position: [0.0, 1.1, 0.2],
            rotation: [0.0, 0.0, 0.0, 1.0],
            curve: Some(0.2),
        }),
    };
    let sleeve = Piece {
        id: PieceId(2),
        name: "Sleeve".into(),
        vertices: vec![
            corner(400.0, 0.0),
            corner(700.0, 0.0),
            corner(700.0, 200.0),
            corner(400.0, 200.0),
        ],
        edges: vec![
            Edge::Line,
            Edge::Line,
            Edge::Curve {
                c1: at(650.0, 320.0),
                c2: at(450.0, 320.0),
            },
            Edge::Line,
        ],
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true, 4),
        notches: vec![Notch::new(2, 180.0)],
        lines: vec![],
        fold: None,
        twin: None, // set below, once its id is taken
        placement: Some(Placement {
            position: [0.25, 1.15, 0.0],
            rotation: [0.0, 0.0, 0.382_683_432_365_089_8, 0.923_879_532_511_286_7],
            curve: Some(0.08),
        }),
    };
    let mut expected = Project::new();
    expected.add_piece(front);
    let sleeve = expected.add_piece(sleeve);
    expected
        .add_twin(sleeve, "Sleeve (mirror)".into(), at(1500.0, 0.0))
        .unwrap();
    let side =
        |shape: u32, half: Half, from: (usize, f64), to: (usize, f64), forward: bool| SeamSide {
            shape: PieceId(shape),
            half,
            from: OutlinePos::new(from.0, from.1),
            to: OutlinePos::new(to.0, to.1),
            forward,
        };
    expected.seams = vec![
        Seam {
            id: SeamId(1),
            a: side(1, Half::Drawn, (1, 0.0), (1, 1.0), true),
            b: side(2, Half::Drawn, (1, 1.0), (1, 0.0), false),
        },
        Seam {
            id: SeamId(2),
            a: side(2, Half::Drawn, (2, 0.0), (2, 0.5), true),
            b: side(1, Half::Drawn, (2, 0.0), (2, 1.0), true),
        },
        Seam {
            id: SeamId(3),
            a: side(1, Half::Pale, (0, 0.25), (0, 0.75), true),
            b: side(3, Half::Drawn, (0, 1.0), (0, 0.0), false),
        },
    ];
    let pin = |shape: u32, half: Half, x: f64, y: f64, target: [f64; 3]| Pin {
        shape: PieceId(shape),
        half,
        at: at(x, y),
        target,
    };
    expected.pins = vec![
        pin(1, Half::Drawn, 100.0, 100.0, [0.05, 1.0, 0.25]),
        pin(1, Half::Pale, 50.0, 200.0, [-0.05, 1.0, 0.25]),
        pin(3, Half::Drawn, 600.0, 100.0, [-0.3, 1.1, 0.0]),
    ];
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 4);
    // Each of the three seams has a mirror image.
    assert_eq!(loaded.all_seams().len(), 6);
}

#[test]
fn refuses_invalid_v4_details() {
    let good = include_str!("fixtures/v4/project.json");
    for (from, to) in [
        (
            r#""to": { "edge": 2, "t": 0.5 }"#,
            r#""to": { "edge": 2, "t": 1.5 }"#,
        ), // past the end of the cap
        (
            r#""from": { "edge": 0, "t": 0.25 }, "to": { "edge": 0, "t": 0.75 }"#,
            r#""from": { "edge": 0, "t": 0.25 }, "to": { "edge": 0, "t": 0.251 }"#,
        ), // 0.05 mm long
        (
            r#""shape": 1, "half": "pale", "from": { "edge": 0, "t": 0.25 }"#,
            r#""shape": 1, "half": "pale", "from": { "edge": 1, "t": 0.25 }"#,
        ), // round to the pale side edge, which the first seam's mirror image sews
        (
            r#""at": { "x": 600.0, "y": 100.0 }"#,
            r#""at": { "x": 720.0, "y": 100.0 }"#,
        ), // 20 mm outside the sleeve
        (
            r#""target": [-0.3, 1.1, 0.0]"#,
            r#""target": [-0.3, 11.1, 0.0]"#,
        ), // held 11 m up
        (r#""shape": 3, "at""#, r#""shape": 3, "half": "pale", "at""#), // the twin has no pale half
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
