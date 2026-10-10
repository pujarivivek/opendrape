//! Everything M2b, M4a and M4b added to a project (seam allowances, hems, notches, internal
//! lines, folds, mirrored pairs, seams, 3D placements, free seam sides and pins) must survive
//! saving and opening exactly. The project is built field by field here, with no default left
//! where a value could be lost without anyone noticing.

use opendrape_core::{
    Edge, EdgeProps, Half, InternalLine, LineKind, Notch, NotchStyle, OutlinePos, Piece, PieceId,
    Pin, Placement, Point2, Project, SeamSide, Units, Vertex, VertexKind,
};

fn at(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// A bodice that is paired with a mirror image, and a sleeve cut on the fold.
fn detailed_project() -> Project {
    let mut project = Project::new();
    project.units = Units::Inch;

    let mut bodice = Piece::rectangle(PieceId(0), "Bodice", at(10.5, 20.25), 400.0, 600.0);
    bodice.grain_deg = 75.5;
    bodice.allowance = 12.0; // the whole piece
    bodice.edge_props = vec![
        EdgeProps {
            allowance: Some(5.5), // one edge's own
            hem: false,
        },
        EdgeProps {
            allowance: None,
            hem: true, // a hem with the hem's 3 cm
        },
        EdgeProps {
            allowance: Some(0.0), // none at all, and a hem
            hem: true,
        },
        EdgeProps::default(),
    ];
    bodice.edges[3] = Edge::Curve {
        c1: at(-40.5, 450.0),
        c2: at(-30.0, 150.75),
    };
    bodice.vertices[2].kind = VertexKind::Smooth;
    let notch = |edge, distance, marks, style| Notch {
        edge,
        distance,
        marks,
        style,
    };
    bodice.notches = vec![
        notch(0, 50.5, 1, NotchStyle::Slit),
        notch(0, 120.25, 2, NotchStyle::V),
        notch(1, 200.0, 3, NotchStyle::Slit),
        notch(2, 33.125, 3, NotchStyle::V),
        notch(3, 75.0, 2, NotchStyle::Slit),
    ];
    bodice.lines = vec![
        // An open marking line with a curve through a smooth point.
        InternalLine {
            vertices: vec![
                Vertex::corner(at(60.0, 120.0)),
                Vertex::smooth(at(200.0, 320.0)),
                Vertex::corner(at(340.0, 120.0)),
            ],
            edges: vec![
                Edge::Curve {
                    c1: at(90.0, 260.0),
                    c2: at(150.0, 330.0),
                },
                Edge::Line,
            ],
            closed: false,
            kind: LineKind::Marking,
        },
        // A closed cut-out with one curved edge.
        InternalLine {
            vertices: vec![
                Vertex::corner(at(150.0, 400.0)),
                Vertex::corner(at(250.0, 400.0)),
                Vertex::corner(at(200.0, 500.0)),
            ],
            edges: vec![
                Edge::Curve {
                    c1: at(180.0, 380.0),
                    c2: at(220.0, 380.0),
                },
                Edge::Line,
                Edge::Line,
            ],
            closed: true,
            kind: LineKind::Cutout,
        },
        // And a closed marking line.
        InternalLine::polygon(&[at(100.0, 200.0), at(140.0, 200.0), at(120.0, 240.0)]),
    ];
    let bodice_id = project.add_piece(bodice);
    // The mirror image sits higher than its piece: its offset has a height.
    let twin = project
        .add_twin(bodice_id, "Bodice (mirror)".into(), at(905.5, 25.75))
        .expect("a piece without a fold takes a twin");
    assert!(twin.0 > bodice_id.0);

    // Half a sleeve, with the left edge as its fold.
    let mut sleeve = Piece::rectangle(PieceId(0), "Sleeve", at(1500.0, 0.0), 200.0, 500.0);
    sleeve.fold = Some(3);
    sleeve.allowance = 0.0;
    sleeve.edge_props[1].hem = true;
    sleeve.notches = vec![notch(0, 80.0, 2, NotchStyle::V)];
    sleeve.lines = vec![InternalLine::open(&[at(1550.0, 100.0), at(1650.0, 300.0)])];
    sleeve.placement = Some(Placement {
        position: [0.31, 1.12, -0.05],
        rotation: [0.0, 0.707_106_781_186_547_5, 0.0, 0.707_106_781_186_547_5],
        curve: Some(0.075),
    });
    let sleeve_id = project.add_piece(sleeve);

    // The bodice's right edge to the sleeve's pale half, run backwards; and the bodice's left
    // and bottom edges (wrapping past the last edge) to the sleeve's drawn bottom edge.
    project.add_seam(
        SeamSide::edges(bodice_id, Half::Drawn, 1, 1, true),
        SeamSide::edges(sleeve_id, Half::Pale, 1, 1, false),
    );
    project.add_seam(
        SeamSide::edges(bodice_id, Half::Drawn, 3, 0, true),
        SeamSide::edges(sleeve_id, Half::Drawn, 0, 0, true),
    );
    // Part of the bodice's top, run backwards, to most of the sleeve's top.
    project.add_seam(
        SeamSide {
            from: OutlinePos::new(2, 0.7),
            to: OutlinePos::new(2, 0.2),
            ..SeamSide::edges(bodice_id, Half::Drawn, 2, 2, false)
        },
        SeamSide {
            from: OutlinePos::new(2, 0.1),
            to: OutlinePos::new(2, 0.9),
            ..SeamSide::edges(sleeve_id, Half::Drawn, 2, 2, true)
        },
    );
    // Pins on the bodice, the sleeve's pale half and the bodice's mirror image.
    project.pins = vec![
        Pin {
            shape: bodice_id,
            half: Half::Drawn,
            at: at(200.5, 300.25),
            target: [0.12, 1.31, 0.42],
        },
        Pin {
            shape: sleeve_id,
            half: Half::Pale,
            at: at(1550.0, 250.0),
            target: [0.4, 1.1, -0.05],
        },
        Pin {
            shape: twin,
            half: Half::Drawn,
            at: at(100.0, 500.0),
            target: [-0.2, 1.2, 0.3],
        },
    ];
    project.set_placement(bodice_id, Some(Placement::at([0.0, 1.25, 0.4])));
    project.set_placement(
        twin,
        Some(Placement {
            position: [-0.12, 1.25, 0.15],
            rotation: [0.0, 0.0, 0.258_819_045_102_520_74, 0.965_925_826_289_068_3],
            curve: None,
        }),
    );

    assert_eq!(project.check(), Ok(()), "the sample is a valid project");
    project
}

#[test]
fn every_m2b_m4a_and_m4b_field_survives_a_save_and_open() {
    let project = detailed_project();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("detailed.odp");
    opendrape_io::save(&project, &file).unwrap();
    let loaded = opendrape_io::load(&file).unwrap();
    assert_eq!(loaded, project);
    // Saving what was opened gives the same project again, byte for byte.
    let again = dir.path().join("again.odp");
    opendrape_io::save(&loaded, &again).unwrap();
    assert_eq!(opendrape_io::load(&again).unwrap(), project);
    assert_eq!(
        opendrape_io::to_bytes(&loaded).unwrap(),
        opendrape_io::to_bytes(&project).unwrap()
    );
}

#[test]
fn the_sample_really_uses_every_new_feature() {
    // So that the round trip above can't pass by leaving something out of the sample.
    let project = detailed_project();
    let bodice = &project.pieces[0];
    assert_eq!(bodice.allowance, 12.0);
    assert!(bodice.edge_props.iter().any(|e| e.allowance == Some(5.5)));
    assert!(bodice.edge_props.iter().any(|e| e.hem));
    for style in [NotchStyle::Slit, NotchStyle::V] {
        assert!(bodice.notches.iter().any(|n| n.style == style));
    }
    for marks in 1..=3 {
        assert!(bodice.notches.iter().any(|n| n.marks == marks));
    }
    assert!(
        bodice
            .lines
            .iter()
            .any(|l| !l.closed && l.kind == LineKind::Marking)
    );
    assert!(
        bodice
            .lines
            .iter()
            .any(|l| l.closed && l.kind == LineKind::Cutout)
    );
    assert!(
        bodice
            .lines
            .iter()
            .any(|l| l.edges.iter().any(|e| matches!(e, Edge::Curve { .. })))
    );
    assert_ne!(bodice.twin.as_ref().unwrap().offset.y, 0.0);
    assert_eq!(project.pieces[1].fold, Some(3));
    // M4a: seams on both halves, running both ways, one wrapping past the last edge, and
    // placements on a piece, a twin and a curved piece.
    let sides: Vec<SeamSide> = project.seams.iter().flat_map(|s| [s.a, s.b]).collect();
    assert!(sides.iter().any(|s| s.half == Half::Pale) && sides.iter().any(|s| !s.forward));
    assert!(sides.iter().any(|s| s.to.edge < s.from.edge), "wraps");
    assert!(bodice.placement.is_some() && bodice.twin.as_ref().unwrap().placement.is_some());
    assert!(
        project.pieces[1]
            .placement
            .is_some_and(|p| p.curve.is_some())
    );
    assert_eq!(
        project.all_seams().len(),
        6,
        "every seam has a mirror image"
    );
    // M4b: a side ending part-way along an edge, and pins on a piece, a pale half and a twin.
    assert!(
        sides
            .iter()
            .any(|s| ![0.0, 1.0].contains(&s.from.t) && ![0.0, 1.0].contains(&s.to.t))
    );
    let pinned: Vec<(PieceId, Half)> = project.pins.iter().map(|p| (p.shape, p.half)).collect();
    assert_eq!(
        pinned,
        vec![
            (PieceId(1), Half::Drawn),
            (PieceId(3), Half::Pale),
            (PieceId(2), Half::Drawn)
        ]
    );
}
