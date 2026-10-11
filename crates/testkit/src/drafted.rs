//! Garments drafted the way a student would draft them, placed on a stage the way the app
//! places them: the drape gates and the benchmark share them.

use opendrape_core::{
    Edge, Half, Notch, OutlinePos, Piece, PieceId, Placement, Point2, Project, SeamSide,
};
use opendrape_drape::Stage;
use opendrape_geom as geom;
use opendrape_mesh::place::PlaceAt;

/// Where the skirt's waist goes, as for M1's demo skirt (m).
pub const SKIRT_WAIST_Y: f64 = 1.03;
/// The skirt's length (mm).
pub const SKIRT_LENGTH_MM: f64 = 550.0;
/// A quarter of the skirt's waist and of its hem (mm): the front half's width at each (the
/// fold is the other half). Drafted for the default form's hips (93 cm), with room over them.
pub const SKIRT_WAIST_QUARTER_MM: f64 = 177.5;
pub const SKIRT_HEM_QUARTER_MM: f64 = 440.0;
/// Where the bodice's neck points go (m): on the base of the neck.
pub const NECK_Y: f64 = 1.36;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// The skirt: a front half on the fold (hem 440, waist 177.5, 550 long, the fold its left edge),
/// a "Back left" (side seam slanted on its left, centre back straight on its right) and its
/// mirror image to its right. Side seams: the front's right edge to the back's slanted edge,
/// both starting at the hem (the mirror image sews the other side). Centre back: the back to
/// its twin. Every piece is moved down to waist height, then Placed at front or back.
pub fn skirt(stage: &Stage) -> Project {
    let mut pr = Project::new();
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(440.0, 0.0), p(177.5, 550.0), p(0.0, 550.0)],
    );
    front.fold = Some(3);
    let front = pr.add_piece(front);
    let back = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Back left",
        &[
            p(500.0, 0.0),
            p(940.0, 0.0),
            p(940.0, 550.0),
            p(762.5, 550.0),
        ],
    ));
    let twin = pr
        .add_twin(back, "Back right".into(), p(1500.0, 0.0))
        .unwrap();
    let side = |shape, edge, forward| SeamSide::edges(shape, Half::Drawn, edge, edge, forward);
    pr.add_seam(side(front, 1, true), side(back, 3, false));
    pr.add_seam(side(back, 1, true), side(twin, 1, true));
    assert_eq!(pr.check(), Ok(()));
    assert_eq!(
        pr.all_seams().len(),
        3,
        "the side seam's mirror image sews the other side"
    );
    // Typed in Properties: down to waist height (the middle of each piece).
    let middle = SKIRT_WAIST_Y - SKIRT_LENGTH_MM / 2000.0;
    for id in [front, back] {
        assert!(pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4]))));
    }
    // Place at…, through the one helper the app's menu uses.
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let placed = stage.place_at(&pr, id, at).expect("the piece is there");
        assert!(pr.set_placement(id, Some(placed)));
    }
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// The T-shirt and the ids its gate needs.
pub struct TShirt {
    pub project: Project,
    pub front: PieceId,
    pub sleeve: PieceId,
    pub twin: PieceId,
}

/// A T-shirt: a front and a back cut on the fold, and a mirrored pair of sleeves with a notch
/// at the top of the cap. The shoulders, the sides and the sleeve's underarm are sewn whole-edge
/// (W); each cap is sewn into its armhole in two free seams (F) that meet at the cap notch. The
/// bodice is moved up to the neck (typed), Placed at front and back, and the sleeve Placed at →
/// Left arm (its twin goes on the right arm).
pub fn t_shirt(stage: &Stage) -> TShirt {
    let mut pr = Project::new();
    // The front half, its fold the centre front (its last edge, on the left): hem 250 wide,
    // side 400 long, an armhole curving in to the shoulder point, a sloping shoulder and a
    // scooped neckline.
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[
            p(0.0, 0.0),
            p(250.0, 0.0),
            p(250.0, 400.0),
            p(190.0, 590.0),
            p(75.0, 615.0),
            p(0.0, 540.0),
        ],
    );
    front.edges[2] = Edge::Curve {
        c1: p(210.0, 420.0),
        c2: p(185.0, 520.0),
    };
    front.edges[4] = Edge::Curve {
        c1: p(75.0, 570.0),
        c2: p(40.0, 540.0),
    };
    front.fold = Some(5);
    // The back half, drawn to the left of its fold (the centre back, its edge 1), with a
    // shallower neckline: placed at the back, its drawn half is on the body's left, as the
    // front's is.
    let mut back = Piece::polygon(
        PieceId(0),
        "Back",
        &[
            p(550.0, 0.0),
            p(800.0, 0.0),
            p(800.0, 595.0),
            p(725.0, 615.0),
            p(610.0, 590.0),
            p(550.0, 400.0),
        ],
    );
    back.edges[2] = Edge::Curve {
        c1: p(770.0, 595.0),
        c2: p(725.0, 600.0),
    };
    back.edges[4] = Edge::Curve {
        c1: p(615.0, 520.0),
        c2: p(590.0, 420.0),
    };
    back.fold = Some(1);
    // A sleeve: hem 300, underarm edges 130 long, a cap (edge 2, from the right underarm
    // corner round to the left) 97.5 mm high with a notch at its top.
    let mut sleeve = Piece::polygon(
        PieceId(0),
        "Sleeve",
        &[
            p(920.0, 0.0),
            p(1220.0, 0.0),
            p(1240.0, 130.0),
            p(900.0, 130.0),
        ],
    );
    sleeve.edges[2] = Edge::Curve {
        c1: p(1190.0, 260.0),
        c2: p(950.0, 260.0),
    };
    let cap = geom::edge_length(&sleeve, 2);
    sleeve.notches = vec![Notch::new(2, cap / 2.0)];
    let front = pr.add_piece(front);
    let back = pr.add_piece(back);
    let sleeve = pr.add_piece(sleeve);
    let twin = pr
        .add_twin(sleeve, "Sleeve (mirror)".into(), p(2540.0, 0.0))
        .unwrap();
    let whole = |shape, edge, forward| SeamSide::edges(shape, Half::Drawn, edge, edge, forward);
    let cap_part = |from: f64, to: f64| SeamSide {
        shape: sleeve,
        half: Half::Drawn,
        from: OutlinePos::new(2, from),
        to: OutlinePos::new(2, to),
        forward: false,
    };
    // W: the shoulders (from the shoulder points), the sides (from the hem), and the sleeve's
    // underarm (from the hem).
    pr.add_seam(whole(front, 3, true), whole(back, 3, false));
    pr.add_seam(whole(front, 1, true), whole(back, 5, false));
    pr.add_seam(whole(sleeve, 1, true), whole(sleeve, 3, false));
    // F: the cap from its left underarm corner back to the notch, into the front armhole from
    // the underarm up; then from the notch on to its right corner, into the back armhole from
    // the shoulder down.
    pr.add_seam(cap_part(1.0, 0.5), whole(front, 2, true));
    pr.add_seam(cap_part(0.5, 0.0), whole(back, 4, true));
    assert_eq!(pr.check(), Ok(()));
    assert_eq!(pr.all_seams().len(), 10, "every seam has its mirror image");
    // Typed in Properties: the bodice up, so its neck points sit on the base of the neck.
    let middle = NECK_Y - 0.615 / 2.0;
    for id in [front, back] {
        assert!(pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4]))));
    }
    // Place at front and back, then Place at → Left arm, through the one helper the app's menu
    // uses: the twin, with no placement of its own, mirrors the sleeve's onto the right arm.
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let placed = stage.place_at(&pr, id, at).expect("the piece is there");
        assert!(pr.set_placement(id, Some(placed)));
    }
    let placed = stage
        .place_at_arm(&pr, sleeve, 0)
        .expect("the bundled body has arms");
    assert!(pr.set_placement(sleeve, Some(placed)));
    assert_eq!(pr.check(), Ok(()));
    TShirt {
        project: pr,
        front,
        sleeve,
        twin,
    }
}
