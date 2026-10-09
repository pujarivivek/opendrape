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
    // slanted edge backwards, so it starts at the hem too: each pair starts level.
    for &((pa, a), (pb, b)) in &mesh.stitches[..side_seam] {
        let (fa, fb) = (
            mesh.panels[pa].flat[a as usize],
            mesh.panels[pb].flat[b as usize],
        );
        assert!((fa[1] - fb[1]).abs() < 1e-9, "level: {fa:?} {fb:?}");
    }
    // Every stitched point is on its panel's outline, and no point is sewn twice by a seam.
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
    // Corners of 24° and 36°: the triangles touching them can't beat those angles, but every
    // triangle is at least 20° unless a corner sharper than that is part of it, and every
    // triangle more than 4 h from a corner sharper than 45° is at least 25°.
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
    // The two-edge side keeps its corner: the short piece's corner (100,200) is stitched.
    let short_panel = &mesh.panels[mesh.panel_of(short).unwrap()];
    let corner = short_panel.edges[2][0];
    assert!(mesh.stitches[first..].iter().any(|s| s.0 == (0, corner)));
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
