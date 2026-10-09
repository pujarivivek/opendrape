//! The fabric `opendrape_mesh::build` makes from a student's pattern: well-shaped triangles,
//! the whole stitching outline covered, holes where cut-outs are, and both sides of every seam
//! sampled with the same number of points.

use opendrape_core::{
    Edge, Half, InternalLine, LineKind, Piece, PieceId, Point2, Project, SeamSide,
};
use opendrape_geom as geom;
use opendrape_mesh::{MeshNote, MeshParams, PanelMesh, Stitch, build};

const H: f64 = 12.0;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

fn side(shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool) -> SeamSide {
    SeamSide::new(shape, half, first_edge, edges, forward)
}

/// The skirt a student drafts: a front on the fold (half: hem 300, waist 177.5, 550 long, with
/// a curved waist and hem), and a back left with its mirrored twin. Side seams sewn (the
/// front's right edge to the back's slanted edge) and the centre back (the back to its twin).
/// Returns the project and the ids of front, back and twin.
fn skirt() -> (Project, [PieceId; 3]) {
    let mut pr = Project::new();
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(300.0, 0.0), p(177.5, 550.0), p(0.0, 550.0)],
    );
    front.edges[0] = Edge::Curve {
        c1: p(100.0, -12.0),
        c2: p(200.0, -10.0),
    };
    front.edges[2] = Edge::Curve {
        c1: p(120.0, 556.0),
        c2: p(60.0, 558.0),
    };
    front.fold = Some(3);
    let front = pr.add_piece(front);
    let back = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Back",
        &[
            p(400.0, 0.0),
            p(700.0, 0.0),
            p(700.0, 550.0),
            p(522.5, 550.0),
        ],
    ));
    let twin = pr
        .add_twin(back, "Back (mirror)".into(), p(1500.0, 0.0))
        .unwrap();
    pr.add_seam(
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    pr.add_seam(
        side(back, Half::Drawn, 1, 1, true),
        side(twin, Half::Drawn, 1, 1, true),
    );
    assert_eq!(pr.check(), Ok(()));
    (pr, [front, back, twin])
}

/// A bodice-like piece: an armhole curve, a neckline curve and a round cut-out.
fn bodice() -> Project {
    let mut pr = Project::new();
    let mut piece = Piece::polygon(
        PieceId(0),
        "Bodice",
        &[
            p(0.0, 0.0),
            p(220.0, 0.0),
            p(230.0, 260.0),
            p(150.0, 400.0),
            p(60.0, 420.0),
            p(0.0, 380.0),
        ],
    );
    piece.edges[2] = Edge::Curve {
        c1: p(180.0, 280.0),
        c2: p(170.0, 360.0),
    };
    piece.edges[4] = Edge::Curve {
        c1: p(40.0, 380.0),
        c2: p(10.0, 370.0),
    };
    let ring: Vec<Point2> = (0..24)
        .map(|k| {
            let a = k as f64 / 24.0 * std::f64::consts::TAU;
            p(100.0 + 30.0 * a.cos(), 150.0 + 30.0 * a.sin())
        })
        .collect();
    piece.lines = vec![InternalLine {
        kind: LineKind::Cutout,
        ..InternalLine::polygon(&ring)
    }];
    pr.add_piece(piece);
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// Each triangle of `panel` as three points in millimetres.
fn triangles_mm(panel: &PanelMesh) -> Vec<[Point2; 3]> {
    panel
        .triangles
        .iter()
        .map(|t| {
            t.map(|k| {
                p(
                    panel.flat[k as usize][0] * 1000.0,
                    panel.flat[k as usize][1] * 1000.0,
                )
            })
        })
        .collect()
}

fn angles(t: [Point2; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| {
        let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
        let (u, w) = (b - a, c - a);
        ((u.x * w.x + u.y * w.y) / (u.length() * w.length()))
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    })
}

fn area(t: [Point2; 3]) -> f64 {
    0.5 * ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[2].x - t[0].x) * (t[1].y - t[0].y))
}

#[test]
fn a_skirt_becomes_three_panels_sewn_at_matching_points() {
    let (pr, [front, back, twin]) = skirt();
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![]);
    assert_eq!(
        mesh.panels.iter().map(|p| p.shape).collect::<Vec<_>>(),
        vec![front, back, twin]
    );
    // The side seam (563.5 mm), its mirror image and the centre back (550 mm): each side gets
    // round(length / 12) + 1 points.
    let side_seam = (563.5_f64 / H).round() as usize + 1;
    let centre_back = (550.0 / H).round() as usize + 1;
    assert_eq!(mesh.stitches.len(), 2 * side_seam + centre_back);
    // The front's side seam runs up its right edge from the hem; the back's runs down its
    // slanted edge backwards, so it starts at the hem too. The mirror image (the front's pale
    // half against the twin) and the centre back (the back against its twin) start at the hem
    // as well: every pair of every block starts level and stays level.
    let level = |range: std::ops::Range<usize>, what: &str| {
        for &((pa, a), (pb, b)) in &mesh.stitches[range] {
            let (fa, fb) = (
                mesh.panels[pa].flat[a as usize],
                mesh.panels[pb].flat[b as usize],
            );
            assert!((fa[1] - fb[1]).abs() < 1e-9, "{what} level: {fa:?} {fb:?}");
        }
    };
    level(0..side_seam, "side seam");
    level(side_seam..2 * side_seam, "its mirror image");
    level(2 * side_seam..mesh.stitches.len(), "centre back");
    // The mirror image sews the front's left half to the twin, from the hem.
    let ((pa, a), (pb, b)) = mesh.stitches[side_seam];
    assert_eq!(
        (mesh.panels[pa].shape, mesh.panels[pb].shape),
        (front, twin)
    );
    let (fa, fb) = (
        mesh.panels[pa].flat[a as usize],
        mesh.panels[pb].flat[b as usize],
    );
    assert!(
        fa[0] < 0.0 && fa[1].abs() < 1e-9 && fb[1].abs() < 1e-9,
        "starts at the hem, on the front's left half: {fa:?} {fb:?}"
    );
    // Every stitched point is on its panel's outline.
    for &((pa, a), (pb, b)) in &mesh.stitches {
        assert!(mesh.panels[pa].edges.iter().flatten().any(|&k| k == a));
        assert!(mesh.panels[pb].edges.iter().flatten().any(|&k| k == b));
    }
    // The whole front is one panel with the fold inside it: twice the half's area.
    let half = geom::area(pr.piece(front).unwrap());
    let front_area: f64 = triangles_mm(&mesh.panels[0]).into_iter().map(area).sum();
    assert!(
        (front_area / (2.0 * half) - 1.0).abs() < 0.005,
        "{front_area} vs {}",
        2.0 * half
    );
    // The twin is its own panel, the back's mirror image.
    let back_area: f64 = triangles_mm(&mesh.panels[1]).into_iter().map(area).sum();
    let twin_area: f64 = triangles_mm(&mesh.panels[2]).into_iter().map(area).sum();
    assert!((back_area - twin_area).abs() < 1e-6 * back_area);
}

#[test]
fn triangles_are_well_shaped_and_close_to_the_target_size() {
    let (skirt, _) = skirt();
    for pr in [skirt, bodice()] {
        let mesh = build(&pr, &MeshParams::default());
        for panel in &mesh.panels {
            for t in triangles_mm(panel) {
                let smallest = angles(t).into_iter().fold(180.0, f64::min);
                assert!(smallest >= 25.0, "{smallest:.2}° in {:?}", panel.shape);
                for i in 0..3 {
                    let e = t[i].distance(t[(i + 1) % 3]) / H;
                    assert!(
                        (0.5..=1.6).contains(&e),
                        "an edge of {e:.2} h in {:?}",
                        panel.shape
                    );
                }
            }
        }
    }
}

#[test]
fn sharp_corners_only_spoil_the_triangles_next_to_them() {
    // A spike whose corners are 90°, 31°, 59° and a straight 180°: the triangles touching the
    // 31° corner can't beat it, but every triangle is at least 20° unless a corner sharper
    // than that is part of it (none is, here), and every triangle more than 4 h from a corner
    // sharper than 45° is at least 25°.
    let mut pr = Project::new();
    let corners = [p(0.0, 0.0), p(500.0, 0.0), p(150.0, 210.0), p(0.0, 300.0)];
    let piece = pr.add_piece(Piece::polygon(PieceId(0), "Spike", &corners));
    let mesh = build(&pr, &MeshParams::default());
    let panel = &mesh.panels[mesh.panel_of(piece).unwrap()];
    let corner_angles: Vec<f64> = (0..4)
        .map(|k| {
            let (a, o, b) = (corners[(k + 3) % 4], corners[k], corners[(k + 1) % 4]);
            angles([o, a, b])[0]
        })
        .collect();
    assert!(corner_angles.iter().any(|a| *a < 45.0), "{corner_angles:?}");
    for t in triangles_mm(panel) {
        let smallest = angles(t).into_iter().fold(180.0, f64::min);
        let near_sharp = corners
            .iter()
            .zip(&corner_angles)
            .any(|(c, a)| *a < 45.0 && t.iter().any(|q| q.distance(*c) < 4.0 * H));
        if !near_sharp {
            assert!(
                smallest >= 25.0,
                "{smallest:.2}° away from the sharp corners"
            );
        }
        let touches_sharper_than_20 = corners
            .iter()
            .zip(&corner_angles)
            .any(|(c, a)| *a < 20.0 && t.iter().any(|q| q.distance(*c) < 1e-9));
        if !touches_sharper_than_20 {
            assert!(smallest >= 20.0, "{smallest:.2}°");
        }
    }
}

#[test]
fn the_fabric_covers_the_stitching_outline_except_cut_outs() {
    let pr = bodice();
    let mesh = build(&pr, &MeshParams::default());
    let panel = &mesh.panels[0];
    let piece = &pr.pieces[0];
    let hole = 30.0 * 30.0 * std::f64::consts::PI;
    let want = geom::area(piece) - hole;
    let got: f64 = triangles_mm(panel).into_iter().map(area).sum();
    assert!((got / want - 1.0).abs() < 0.005, "{got} vs {want}");
    for t in triangles_mm(panel) {
        let c = p(
            (t[0].x + t[1].x + t[2].x) / 3.0,
            (t[0].y + t[1].y + t[2].y) / 3.0,
        );
        assert!(
            c.distance(p(100.0, 150.0)) > 29.0,
            "a triangle in the cut-out at {c:?}"
        );
    }
}

#[test]
fn seam_sides_always_get_the_same_count() {
    let mut pr = Project::new();
    let short = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Short",
        p(0.0, 0.0),
        100.0,
        200.0,
    ));
    let long = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Long",
        p(300.0, 0.0),
        100.0,
        236.0,
    ));
    // The short piece's right edge (200 mm) to the long one's left (236 mm), and the short
    // piece's top and left edges (300 mm, two edges) to the long one's bottom and right (336).
    let ease = pr.add_seam(
        side(short, Half::Drawn, 1, 1, true),
        side(long, Half::Drawn, 3, 1, false),
    );
    pr.add_seam(
        side(short, Half::Drawn, 2, 2, true),
        side(long, Half::Drawn, 0, 2, false),
    );
    let mesh = build(&pr, &MeshParams::default());
    assert!(mesh.notes.iter().any(|n| matches!(
        n,
        MeshNote::LengthsDiffer { seam, by_mm } if *seam == ease && (by_mm - 36.0).abs() < 1e-6
    )));
    let first = (236.0_f64 / H).round() as usize + 1;
    let second = (336.0_f64 / H).round() as usize + 1;
    assert_eq!(mesh.stitches.len(), first + second);
    // Each side's points are all different points.
    for range in [0..first, first..first + second] {
        let distinct = |pick: fn(&Stitch) -> u32| {
            let mut points: Vec<u32> = mesh.stitches[range.clone()].iter().map(pick).collect();
            points.sort_unstable();
            points.dedup();
            points.len()
        };
        assert_eq!(distinct(|s| s.0.1), range.len());
        assert_eq!(distinct(|s| s.1.1), range.len());
    }
    // The two-edge side keeps its corner. The short piece's side runs (100,200) → (0,200) →
    // (0,200..0): its corner inside the side is (0,200), vertex 3, 100 mm from the side's
    // start. The 28 steps are 10.71 mm apart on the short side, so the nearest sample (the
    // 9th) is 3.6 mm from the corner: the corner takes it over.
    let short_panel = &mesh.panels[mesh.panel_of(short).unwrap()];
    let long_panel = &mesh.panels[mesh.panel_of(long).unwrap()];
    let corner = short_panel.edges[3][0];
    assert_eq!(
        short_panel.flat[corner as usize],
        [0.0, 0.2],
        "the interior corner"
    );
    let pair = mesh.stitches[first..]
        .iter()
        .find(|s| s.0 == (0, corner))
        .expect("the corner is stitched");
    // Its partner is the 9th step along the long side, which starts at (400,236) and runs
    // down: 9 × 336 / 28 = 108 mm down, at (400,128).
    let partner = long_panel.flat[pair.1.1 as usize];
    assert!(
        (partner[0] - 0.4).abs() < 1e-9 && (partner[1] - 0.128).abs() < 1e-9,
        "{partner:?}"
    );
    // And no point of the short side is left a sliver (under half a step) from the corner.
    let side_points: Vec<[f64; 2]> = mesh.stitches[first..]
        .iter()
        .filter(|s| s.0.0 == 0)
        .map(|s| short_panel.flat[s.0.1 as usize])
        .collect();
    for pair in side_points.windows(2) {
        let gap = (pair[0][0] - pair[1][0]).hypot(pair[0][1] - pair[1][1]) * 1000.0;
        assert!(gap > 0.5 * 10.7, "samples {gap:.2} mm apart");
    }
}

#[test]
fn a_folded_piece_sewn_to_itself_is_one_tube() {
    let mut pr = Project::new();
    let mut half = Piece::rectangle(PieceId(0), "Sleeve", p(0.0, 0.0), 150.0, 400.0);
    half.fold = Some(3);
    let id = pr.add_piece(half);
    // The drawn half's right edge to the pale half's: the sleeve's underarm seam.
    pr.add_seam(
        side(id, Half::Drawn, 1, 1, true),
        side(id, Half::Pale, 1, 1, true),
    );
    assert_eq!(pr.all_seams().len(), 1, "its own mirror image");
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.panels.len(), 1);
    assert_eq!(mesh.stitches.len(), (400.0_f64 / H).round() as usize + 1);
    for &((_, a), (_, b)) in &mesh.stitches {
        let (fa, fb) = (
            mesh.panels[0].flat[a as usize],
            mesh.panels[0].flat[b as usize],
        );
        assert!(
            (fa[0] + fb[0]).abs() < 1e-9 && (fa[1] - fb[1]).abs() < 1e-9,
            "mirror points"
        );
    }
}

#[test]
fn an_outline_that_crosses_itself_is_left_out_with_its_seams() {
    let mut pr = Project::new();
    let bow = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(200.0, 200.0), p(200.0, 0.0), p(0.0, 200.0)],
    ));
    let other = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Back",
        p(400.0, 0.0),
        200.0,
        200.0,
    ));
    // The bow's edge 1, (200,200) to (200,0), is as long as the back's bottom edge.
    pr.add_seam(
        side(bow, Half::Drawn, 1, 1, true),
        side(other, Half::Drawn, 0, 1, true),
    );
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CrossesItself(bow)]);
    assert_eq!(mesh.panels.len(), 1);
    assert_eq!(mesh.panels[0].shape, other);
    assert!(mesh.stitches.is_empty());
}

/// A side's polyline (mm) and the arc length (mm) along it of point `q`, which is on it.
fn arc_along(polyline: &[Point2], q: Point2) -> f64 {
    let mut before = 0.0;
    let mut best = (f64::MAX, 0.0);
    for w in polyline.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        let t =
            (((q.x - a.x) * (b.x - a.x) + (q.y - a.y) * (b.y - a.y)) / (len * len)).clamp(0.0, 1.0);
        let foot = a.lerp(b, t);
        let d = q.distance(foot);
        if d < best.0 {
            best = (d, before + t * len);
        }
        before += len;
    }
    best.1
}

#[test]
fn multi_edge_and_wrapping_sides_pair_start_to_start() {
    // Two 100 × 200 rectangles; the first is sewn along its top and left edges, the second
    // along edges that wrap past its last edge, each both ways round.
    let rect = |pr: &mut Project, x: f64| {
        pr.add_piece(Piece::rectangle(PieceId(0), "R", p(x, 0.0), 100.0, 200.0))
    };
    // (first piece's first edge, second's first edge, second runs forward)
    for (first_a, first_b, forward_b) in [
        (2, 2, true),
        (2, 2, false),
        (3, 3, true),
        (3, 3, false),
        (2, 3, true),
        (3, 2, false),
    ] {
        let mut pr = Project::new();
        let a = rect(&mut pr, 0.0);
        let b = rect(&mut pr, 300.0);
        pr.add_seam(
            side(a, Half::Drawn, first_a, 2, true),
            side(b, Half::Drawn, first_b, 2, forward_b),
        );
        assert_eq!(pr.check(), Ok(()));
        let mesh = build(&pr, &MeshParams::default());
        assert_eq!(mesh.notes, vec![]);
        // Each side as the points it runs through, from its start (corners (0,0) (100,0)
        // (100,200) (0,200), plus the second piece's 300 mm).
        let corner =
            |x: f64, k: usize| p(x + [0.0, 100.0, 100.0, 0.0][k], [0.0, 0.0, 200.0, 200.0][k]);
        let run = |x: f64, first: usize, forward: bool| {
            let mut v = vec![
                corner(x, first),
                corner(x, (first + 1) % 4),
                corner(x, (first + 2) % 4),
            ];
            if !forward {
                v.reverse();
            }
            v
        };
        let (side_a, side_b) = (run(0.0, first_a, true), run(300.0, first_b, forward_b));
        let steps = (300.0_f64 / H).round() as usize;
        assert_eq!(mesh.stitches.len(), steps + 1);
        for (k, &((pa, a), (pb, b))) in mesh.stitches.iter().enumerate() {
            let at = |panel: usize, i: u32| {
                let f = mesh.panels[panel].flat[i as usize];
                p(f[0] * 1000.0, f[1] * 1000.0)
            };
            let (qa, qb) = (at(pa, a), at(pb, b));
            let (sa, sb) = (arc_along(&side_a, qa), arc_along(&side_b, qb));
            // The k-th sample of each side, up to half a step where a corner took one over.
            let want = 300.0 * k as f64 / steps as f64;
            let half_step = 0.5 * 300.0 / steps as f64;
            assert!((sa - want).abs() <= half_step + 1e-6, "a: {sa} vs {want}");
            assert!((sb - want).abs() <= half_step + 1e-6, "b: {sb} vs {want}");
        }
        // The first pair is the two starts, the last the two ends.
        let ((pa, a), (pb, b)) = mesh.stitches[0];
        assert_eq!(
            (
                mesh.panels[pa].flat[a as usize],
                mesh.panels[pb].flat[b as usize]
            ),
            (
                [side_a[0].x / 1000.0, side_a[0].y / 1000.0],
                [side_b[0].x / 1000.0, side_b[0].y / 1000.0]
            )
        );
    }
}

/// A 300 × 300 mm piece with these cut-outs.
fn with_cutouts(lines: Vec<InternalLine>) -> (Project, PieceId) {
    let mut pr = Project::new();
    let mut piece = Piece::rectangle(PieceId(0), "Front", p(0.0, 0.0), 300.0, 300.0);
    piece.lines = lines;
    let id = pr.add_piece(piece);
    assert_eq!(pr.check(), Ok(()));
    (pr, id)
}

fn circle(cx: f64, cy: f64, r: f64) -> InternalLine {
    let ring: Vec<Point2> = (0..24)
        .map(|k| {
            let a = k as f64 / 24.0 * std::f64::consts::TAU;
            p(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    InternalLine {
        kind: LineKind::Cutout,
        ..InternalLine::polygon(&ring)
    }
}

fn cutout(points: &[Point2]) -> InternalLine {
    InternalLine {
        kind: LineKind::Cutout,
        ..InternalLine::polygon(points)
    }
}

/// The meshed area (mm²) of a project's first panel.
fn meshed_area(mesh: &opendrape_mesh::GarmentMesh) -> f64 {
    triangles_mm(&mesh.panels[0]).into_iter().map(area).sum()
}

/// Nothing of the first panel is inside the circle at (cx, cy), radius r.
fn assert_hole_at(mesh: &opendrape_mesh::GarmentMesh, cx: f64, cy: f64, r: f64) {
    for t in triangles_mm(&mesh.panels[0]) {
        let c = p(
            (t[0].x + t[1].x + t[2].x) / 3.0,
            (t[0].y + t[1].y + t[2].y) / 3.0,
        );
        assert!(
            c.distance(p(cx, cy)) > r - 1.0,
            "a triangle in the hole at {c:?}"
        );
    }
}

#[test]
fn a_cut_out_that_touches_or_nears_the_outline_is_left_out_not_the_whole_piece() {
    let whole = 300.0 * 300.0;
    let hole = |r: f64| std::f64::consts::PI * r * r;
    // Touching: a square cut-out sharing a stretch of the right-hand edge.
    let touching = cutout(&[
        p(270.0, 100.0),
        p(300.0, 100.0),
        p(300.0, 140.0),
        p(270.0, 140.0),
    ]);
    // Close: a round one 0.5 mm from the left edge (less than a quarter of 12 mm).
    let close = circle(30.5, 150.0, 30.0);
    // Clear: a round one 5 mm from the top edge, and a round one in the middle.
    let clear_by_5 = circle(150.0, 265.0, 30.0);
    let middle = circle(150.0, 150.0, 30.0);
    let (pr, id) = with_cutouts(vec![touching, close, clear_by_5, middle]);
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CutoutLeftOut(id)], "one note");
    assert_eq!(mesh.panels.len(), 1, "the piece is still made");
    let want = whole - 2.0 * hole(30.0);
    assert!(
        (meshed_area(&mesh) / want - 1.0).abs() < 0.01,
        "{} vs {want}",
        meshed_area(&mesh)
    );
    assert_hole_at(&mesh, 150.0, 150.0, 30.0);
    assert_hole_at(&mesh, 150.0, 265.0, 30.0);
    // Both kept holes are made of good triangles: the 5 mm gap is under half a step.
    for t in triangles_mm(&mesh.panels[0]) {
        assert!(angles(t).into_iter().fold(180.0, f64::min) >= 25.0);
    }
}

#[test]
fn cut_outs_that_overlap_or_nest_are_left_out_and_the_rest_still_meshes() {
    let hole = |r: f64| std::f64::consts::PI * r * r;
    // Two overlapping circles, a small one inside a big one, and one good one.
    let (pr, id) = with_cutouts(vec![
        circle(70.0, 70.0, 30.0),
        circle(100.0, 70.0, 30.0),
        circle(220.0, 220.0, 50.0),
        circle(220.0, 220.0, 15.0),
        circle(80.0, 220.0, 30.0),
    ]);
    let mesh = build(&pr, &MeshParams::default());
    // The overlapping pair go, and so does the one inside the big one (it would be an island
    // of fabric in a hole).
    assert_eq!(mesh.notes, vec![MeshNote::CutoutLeftOut(id)]);
    let want = 300.0 * 300.0 - hole(50.0) - hole(30.0);
    assert!(
        (meshed_area(&mesh) / want - 1.0).abs() < 0.01,
        "{}",
        meshed_area(&mesh)
    );
    assert_hole_at(&mesh, 220.0, 220.0, 50.0);
    assert_hole_at(&mesh, 80.0, 220.0, 30.0);
}

#[test]
fn a_cut_out_outside_the_outline_is_left_out_not_made_into_an_island() {
    let plain = {
        let (pr, _) = with_cutouts(vec![circle(150.0, 150.0, 30.0)]);
        build(&pr, &MeshParams::default())
    };
    // The same, plus a 100 mm square cut-out beside the piece, and a round one far below it.
    let (pr, id) = with_cutouts(vec![
        circle(150.0, 150.0, 30.0),
        cutout(&[
            p(400.0, 100.0),
            p(500.0, 100.0),
            p(500.0, 200.0),
            p(400.0, 200.0),
        ]),
        circle(150.0, -400.0, 40.0),
    ]);
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CutoutLeftOut(id)]);
    assert_eq!(mesh.panels.len(), 1);
    assert_eq!(mesh.particles(), plain.particles(), "no phantom fabric");
    assert!((meshed_area(&mesh) / meshed_area(&plain) - 1.0).abs() < 1e-9);
}

#[test]
fn a_half_cut_out_on_the_fold_leaves_the_whole_piece_made() {
    // A half round cut-out whose diameter lies on the fold: its mirror image shares that
    // diameter, so together they touch. They are left out (with a note) rather than the piece.
    let half: Vec<Point2> = (0..=12)
        .map(|k| {
            let a = -std::f64::consts::FRAC_PI_2 + k as f64 / 12.0 * std::f64::consts::PI;
            p(40.0 * a.cos(), 150.0 + 40.0 * a.sin())
        })
        .collect();
    let mut pr = Project::new();
    let mut piece = Piece::rectangle(PieceId(0), "Front", p(0.0, 0.0), 150.0, 300.0);
    piece.fold = Some(3);
    piece.lines = vec![cutout(&half)];
    let id = pr.add_piece(piece);
    assert_eq!(pr.check(), Ok(()));
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CutoutLeftOut(id)]);
    assert_eq!(mesh.panels.len(), 1, "the piece is still made");
    assert!((meshed_area(&mesh) / (300.0 * 300.0) - 1.0).abs() < 0.005);
}

#[test]
fn a_cut_out_that_crosses_itself_is_left_out_not_the_piece() {
    let eight = cutout(&[
        p(100.0, 100.0),
        p(200.0, 200.0),
        p(200.0, 100.0),
        p(100.0, 200.0),
    ]);
    let (pr, id) = with_cutouts(vec![eight, circle(250.0, 60.0, 25.0)]);
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CutoutLeftOut(id)]);
    assert_hole_at(&mesh, 250.0, 60.0, 25.0);
}

#[test]
fn a_small_cut_out_keeps_its_shape() {
    // The holes are sampled about 12 mm apart, but at least 16 points round: a 16 mm round
    // cut-out used to mesh as a 64 % hole, and a 6 mm one as a triangle.
    for r in [3.0, 8.0, 12.0] {
        let (pr, _) = with_cutouts(vec![circle(150.0, 150.0, r)]);
        let mesh = build(&pr, &MeshParams::default());
        assert_eq!(mesh.notes, vec![], "r = {r}");
        let hole = 300.0 * 300.0 - meshed_area(&mesh);
        let round = std::f64::consts::PI * r * r;
        assert!(
            hole > 0.93 * round && hole < 1.01 * round,
            "r = {r}: a hole of {hole:.1} mm² for a {round:.1} mm² cut-out"
        );
    }
}

#[test]
fn a_seam_whose_shape_is_left_out_has_no_length_note() {
    let mut pr = Project::new();
    let bow = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(200.0, 200.0), p(200.0, 0.0), p(0.0, 200.0)],
    ));
    let other = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Back",
        p(400.0, 0.0),
        300.0,
        200.0,
    ));
    // The bow's edge 1 is 200 mm; the back's bottom edge is 300 mm.
    pr.add_seam(
        side(bow, Half::Drawn, 1, 1, true),
        side(other, Half::Drawn, 0, 1, true),
    );
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CrossesItself(bow)]);
}

#[test]
fn refinement_removes_the_slivers_the_area_limit_alone_leaves() {
    // An uneven six-sided piece where, without the angle limit, a few triangles come out
    // under 25° away from every sharp corner.
    let corners = [
        p(99.16291487055862, 9.441708782851183),
        p(13.612065817932578, 46.12990333650887),
        p(-111.10880368464898, 157.14314452926118),
        p(-173.94021305581768, -23.85630681798028),
        p(-20.32004340989408, -35.674245002829586),
        p(65.33617211501034, -75.72100444798743),
    ];
    let mut pr = Project::new();
    let id = pr.add_piece(Piece::polygon(PieceId(0), "Uneven", &corners));
    let mesh = build(&pr, &MeshParams::default());
    let panel = &mesh.panels[mesh.panel_of(id).unwrap()];
    let n = corners.len();
    let sharp: Vec<Point2> = (0..n)
        .filter(|&k| angles([corners[k], corners[(k + n - 1) % n], corners[(k + 1) % n]])[0] < 40.0)
        .map(|k| corners[k])
        .collect();
    let mut worst = 180.0_f64;
    for t in triangles_mm(panel) {
        if sharp
            .iter()
            .any(|c| t.iter().any(|q| q.distance(*c) < 30.0))
        {
            continue;
        }
        worst = worst.min(angles(t).into_iter().fold(180.0, f64::min));
    }
    assert!(worst >= 25.0, "{worst:.2}°");
}

#[test]
fn a_short_edge_sewn_to_a_long_one_cannot_overflow_the_particle_budget() {
    // A 100 mm square whose side is sewn to a 3 m strip gets the strip's 250 steps along its
    // own 100 mm, far more points than the estimate (made from areas and perimeters) counts:
    // 870 against 794. With a budget of 800 the first try fits the estimate but not the cap.
    // The strip is made first and the square gets what is left, which is not enough to refine
    // it: the first try is not even over the cap, only unfinished.
    let mut pr = Project::new();
    let b = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Strip",
        p(300.0, 0.0),
        3000.0,
        4.0,
    ));
    let a = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Square",
        p(0.0, 0.0),
        100.0,
        100.0,
    ));
    pr.add_seam(
        side(a, Half::Drawn, 1, 1, true),
        side(b, Half::Drawn, 0, 1, true),
    );
    let at_12 = build(
        &pr,
        &MeshParams {
            edge_mm: H,
            max_particles: 1_000_000,
        },
    );
    assert!(at_12.particles() > 800, "{}", at_12.particles());
    let params = MeshParams {
        edge_mm: H,
        max_particles: 800,
    };
    let mesh = build(&pr, &params);
    assert!(mesh.particles() <= 800, "{} particles", mesh.particles());
    assert!(mesh.edge_mm > H);
    assert!(mesh.notes.contains(&MeshNote::Coarser {
        edge_mm: mesh.edge_mm
    }));
    // Every stitch still pairs real points, and the sides still have equal counts.
    for &((pa, a), (pb, b)) in &mesh.stitches {
        assert!(
            (a as usize) < mesh.panels[pa].flat.len() && (b as usize) < mesh.panels[pb].flat.len()
        );
    }
}

#[test]
fn a_budget_nothing_can_meet_still_ends() {
    // 40 panels cannot fit in 20 particles at any edge length: the fabric is made again a few
    // times and then stands as it is, with a note, instead of looping.
    let mut pr = Project::new();
    for k in 0..40 {
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Panel",
            p(k as f64 * 300.0, 0.0),
            200.0,
            200.0,
        ));
    }
    let mesh = build(
        &pr,
        &MeshParams {
            edge_mm: H,
            max_particles: 20,
        },
    );
    assert_eq!(mesh.panels.len(), 40);
    assert!(mesh.edge_mm > H);
    assert!(mesh.notes.contains(&MeshNote::Coarser {
        edge_mm: mesh.edge_mm
    }));
}

#[test]
fn a_huge_pattern_gets_coarser_fabric() {
    let mut pr = Project::new();
    for k in 0..40 {
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Panel",
            p(k as f64 * 1100.0, 0.0),
            1000.0,
            1000.0,
        ));
    }
    let mesh = build(&pr, &MeshParams::default());
    assert!(mesh.edge_mm > H);
    assert_eq!(
        mesh.notes,
        vec![MeshNote::Coarser {
            edge_mm: mesh.edge_mm
        }]
    );
    assert!(
        mesh.particles() < opendrape_mesh::MAX_PARTICLES,
        "{}",
        mesh.particles()
    );
}

#[test]
fn the_same_project_gives_the_same_fabric() {
    let (pr, _) = skirt();
    assert_eq!(
        build(&pr, &MeshParams::default()),
        build(&pr, &MeshParams::default())
    );
}

#[test]
fn random_patterns_never_panic() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut meshed, mut left_out) = (0, 0);
    for _ in 0..150 {
        let mut pr = Project::new();
        let pieces = 1 + (rnd() * 3.0) as usize;
        for k in 0..pieces {
            let n = 3 + (rnd() * 6.0) as usize;
            let corners: Vec<Point2> = (0..n)
                .map(|_| p(k as f64 * 700.0 + rnd() * 500.0, rnd() * 500.0))
                .collect();
            let mut piece = Piece::polygon(PieceId(0), "Random", &corners);
            if rnd() < 0.3 {
                piece.set_curved(0, true);
            }
            let id = pr.add_piece(piece);
            if rnd() < 0.3 {
                pr.add_twin(id, "Twin".into(), p(k as f64 * 700.0 + 2000.0, 0.0));
            }
        }
        let ids: Vec<(PieceId, usize)> = geom::shapes(&pr)
            .iter()
            .map(|s| (s.id, s.stored_len()))
            .collect();
        for _ in 0..3 {
            let (a, na) = ids[(rnd() * ids.len() as f64) as usize];
            let (b, nb) = ids[(rnd() * ids.len() as f64) as usize];
            let mut tried = pr.clone();
            tried.add_seam(
                side(a, Half::Drawn, (rnd() * na as f64) as usize, 1, rnd() < 0.5),
                side(b, Half::Drawn, (rnd() * nb as f64) as usize, 1, rnd() < 0.5),
            );
            if tried.check().is_ok() {
                pr = tried;
            }
        }
        let mesh = build(&pr, &MeshParams::default());
        for panel in &mesh.panels {
            assert!(
                panel
                    .triangles
                    .iter()
                    .flatten()
                    .all(|&k| (k as usize) < panel.flat.len())
            );
        }
        for &((pa, a), (pb, b)) in &mesh.stitches {
            assert!(
                (a as usize) < mesh.panels[pa].flat.len()
                    && (b as usize) < mesh.panels[pb].flat.len()
            );
        }
        meshed += mesh.panels.len();
        left_out += mesh
            .notes
            .iter()
            .filter(|n| matches!(n, MeshNote::CrossesItself(_)))
            .count();
    }
    assert!(
        meshed > 0 && left_out > 0,
        "{meshed} meshed, {left_out} left out"
    );
}
