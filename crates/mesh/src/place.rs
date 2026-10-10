//! Pieces in 3D, in the form's frame (metres, y up, the form faces +z, its left is +x, its
//! centre line at x = 0, z = 0): where a placement puts each flat point, where a piece starts
//! when nobody placed it, the mirror image a twin takes, Place at… (round the body, or round an
//! arm), and the angles typed in Properties.

use glam::{DMat3, DQuat, DVec3};
use opendrape_core::{MAX_CURVE_M, MIN_CURVE_M, Placement, Point2, Project};
use opendrape_geom::{self as geom, Shape, ShapeKind};

/// How far in front of the form's centre line the pattern starts (m).
pub const START_DISTANCE_M: f64 = 0.40;
/// The gap Place at… leaves between the form and a piece (m).
pub const PLACE_GAP_M: f64 = 0.03;
/// Place at… curves a piece this much (m) when no ray finds the form near it.
pub const FALLBACK_RADIUS_M: f64 = 0.2;
/// Place at… measures the form at 25 angles across the span a piece covers...
const ANGLE_STEPS: usize = 24;
/// ...and at heights this far apart (m) over the piece's height.
const ROW_SPACING_M: f64 = 0.02;
/// How many times it measures again at the radius it found.
const RADIUS_TRIES: usize = 3;
/// Place at → arm curves a piece this much (m) when no ray finds the arm near it.
pub const ARM_FALLBACK_RADIUS_M: f64 = 0.08;
/// Where round its arm a sleeve is centred (radians from the arm's front, see [`Arm::around`]):
/// its outer side, so its two underarm edges meet under the arm, facing the body.
pub const SLEEVE_ANGLE: f64 = std::f64::consts::FRAC_PI_2;
/// Place at → arm moves a piece down its arm in steps this long (m) until it is clear of the
/// form...
const ARM_STEP_M: f64 = 0.01;
/// ...at most this far (m).
const ARM_MAX_DROP_M: f64 = 0.2;
/// It checks the piece's points this far apart (mm) for being inside the form, or further
/// apart on a piece more than 40 times as big, so a huge piece is checked as quickly.
const CLEAR_SPACING_MM: f64 = 10.0;

/// The middle of a shape's bounding box on the pattern table (mm): the point a placement puts
/// at its position.
pub fn centre_of(shape: &Shape) -> Point2 {
    let (lo, hi) = crate::bounds(&geom::outline_points(&shape.piece, 0.5));
    lo.lerp(hi, 0.5)
}

pub fn rotation(p: &Placement) -> DQuat {
    DQuat::from_array(p.rotation)
}

pub fn position(p: &Placement) -> DVec3 {
    DVec3::from_array(p.position)
}

/// Where flat point `flat` (mm) of a piece centred on `centre` (mm) goes: centred, wrapped
/// round the placement's cylinder (whose axis lies behind the piece, so its sides bend back),
/// turned, then moved to the placement's position.
pub fn apply(p: &Placement, centre: Point2, flat: Point2) -> DVec3 {
    let (x, y) = ((flat.x - centre.x) / 1000.0, (flat.y - centre.y) / 1000.0);
    let local = match p.curve {
        Some(r) => DVec3::new(r * (x / r).sin(), y, r * (x / r).cos() - r),
        None => DVec3::new(x, y, 0.0),
    };
    rotation(p) * local + position(p)
}

/// The placement that shows a piece's mirror image across x = 0, for its twin: the twin's flat
/// shape is the piece's mirror image, so mirroring the placement too keeps the pair mirrored.
pub fn mirrored(p: &Placement) -> Placement {
    p.mirrored()
}

/// The whole pattern's bounding box on the table (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub min: Point2,
    pub max: Point2,
}

/// The bounding box of every shape's outline.
pub fn layout(shapes: &[Shape]) -> Layout {
    let points: Vec<Point2> = shapes
        .iter()
        .flat_map(|s| geom::outline_points(&s.piece, 0.5))
        .collect();
    let (min, max) = crate::bounds(&points);
    Layout { min, max }
}

/// Where a shape centred on `centre` (mm) starts when nobody has placed it: the pattern table
/// stood up at real size on a vertical plane 40 cm in front of the form, centred left to right
/// on it, with its top at `shoulder_y`.
pub fn start(layout: &Layout, centre: Point2, shoulder_y: f64) -> Placement {
    let middle = (layout.min.x + layout.max.x) / 2.0;
    Placement::at([
        (centre.x - middle) / 1000.0,
        shoulder_y - (layout.max.y - centre.y) / 1000.0,
        START_DISTANCE_M,
    ])
}

/// The placement `shape` is shown and draped with: its own; for a twin with none, its piece's
/// mirrored; otherwise its starting place.
pub fn effective(project: &Project, shape: &Shape, layout: &Layout, shoulder_y: f64) -> Placement {
    if let Some(own) = shape.piece.placement {
        return own;
    }
    if matches!(shape.kind, ShapeKind::Twin { .. })
        && let Some(piece) = project.placement_of(shape.source)
    {
        return mirrored(&piece);
    }
    start(layout, centre_of(shape), shoulder_y)
}

/// The four sides of the form Place at… knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceAt {
    Front,
    Back,
    LeftSide,
    RightSide,
}

impl PlaceAt {
    pub const ALL: [Self; 4] = [Self::Front, Self::Back, Self::LeftSide, Self::RightSide];

    /// Radians round the form from its front towards its left (+x).
    pub fn angle(self) -> f64 {
        use std::f64::consts::{FRAC_PI_2, PI};
        match self {
            Self::Front => 0.0,
            Self::Back => PI,
            Self::LeftSide => FRAC_PI_2,
            Self::RightSide => -FRAC_PI_2,
        }
    }
}

/// Place at…: `shape` wrapped round the form at `at`, at the height it has now.
/// - It curves round a cylinder about the form's centre line, 3 cm clear of the form: the
///   largest surface distance (`surface(angle, y)`, from the centre line, if a ray finds the
///   form there) over the heights the piece spans and the angles it covers, plus the gap.
/// - A cut-on-fold piece (or any unpaired piece) is centred on that angle; a fold drawn
///   upright then lies on the centre line.
/// - A member of a mirrored pair goes beside its partner: its side that faces the partner on
///   the pattern table lies on the centre line at that angle.
pub fn place_at(
    project: &Project,
    shape: &Shape,
    at: PlaceAt,
    layout: &Layout,
    shoulder_y: f64,
    surface: &dyn Fn(f64, f64) -> Option<f64>,
) -> Placement {
    let height = effective(project, shape, layout, shoulder_y).position[1];
    let (lo, hi) = crate::bounds(&geom::outline_points(&shape.piece, 0.5));
    let width = (hi.x - lo.x) / 1000.0;
    let tall = (hi.y - lo.y) / 1000.0;
    let centre = lo.lerp(hi, 0.5);
    // The local x (m) that goes on the centre line: the side facing the partner, or the middle.
    let facing = partner_centre(project, shape).map_or(0.0, |partner| {
        if partner.x > centre.x {
            width / 2.0
        } else {
            -width / 2.0
        }
    });
    let theta = at.angle();
    let rows = ((tall / ROW_SPACING_M).ceil() as usize).max(1);
    // The radius the piece needs when it is wrapped at radius `r`: it covers a span of angles
    // that depends on `r`, and the form is `surface` away across that span and the piece's
    // heights. Wider pieces and tighter curves cover more of the form.
    let need = |r: f64| {
        let middle = theta - facing / r;
        let half_span = width / 2.0 / r;
        let mut farthest: Option<f64> = None;
        for i in 0..=ANGLE_STEPS {
            let angle = middle - half_span + 2.0 * half_span * i as f64 / ANGLE_STEPS as f64;
            for j in 0..=rows {
                let y = height - tall / 2.0 + tall * j as f64 / rows as f64;
                if let Some(d) = surface(angle, y).filter(|d| d.is_finite()) {
                    farthest = Some(farthest.map_or(d, |f: f64| f.max(d)));
                }
            }
        }
        farthest
            .map_or(FALLBACK_RADIUS_M, |d| d + PLACE_GAP_M)
            .clamp(MIN_CURVE_M, MAX_CURVE_M)
    };
    // A larger radius covers a narrower span, so `need` never grows with `r`, and trying again
    // can settle into a two-cycle (a piece that just reaches an arm at the low radius and just
    // misses it at the high one). The larger of the last two tries is always enough for the span
    // it covers: if it is the earlier one, `need` of it is the later; if it is the later one,
    // `need` of it is at most what the earlier one needed, which is the later.
    let (mut previous, mut radius) = (FALLBACK_RADIUS_M, FALLBACK_RADIUS_M);
    for _ in 0..RADIUS_TRIES {
        previous = radius;
        radius = need(radius);
    }
    let radius = radius.max(previous);
    let phi = theta - facing / radius;
    Placement {
        position: [radius * phi.sin(), height, radius * phi.cos()],
        rotation: DQuat::from_rotation_y(phi).to_array(),
        curve: Some(radius),
    }
}

/// One arm of the form, as a straight line down its upper arm: from `shoulder` (at the form's
/// shoulder height) along the unit `direction` for `length` (m). From `free` (m) down the line,
/// past the armpit, the arm hangs free of the body. The arms are each other's mirror image
/// across x = 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    pub shoulder: DVec3,
    pub direction: DVec3,
    pub length: f64,
    pub free: f64,
}

impl Arm {
    /// +1 for the form's left arm (+x), -1 for its right.
    fn side(&self) -> f64 {
        if self.direction.x >= 0.0 { 1.0 } else { -1.0 }
    }

    /// The unit direction square to the arm at `angle` (radians) round it: 0 is the arm's
    /// front (towards +z), π/2 its outer side (away from the body), the same way round on both
    /// arms as their mirror images.
    pub fn around(&self, angle: f64) -> DVec3 {
        let out = (DVec3::Z.cross(self.direction) * self.side()).normalize();
        let front = self.direction.cross(out) * self.side();
        front * angle.cos() + out * angle.sin()
    }

    /// The point `along` metres down the line from the shoulder.
    pub fn at(&self, along: f64) -> DVec3 {
        self.shoulder + self.direction * along
    }

    /// How far (m) `p` is from the arm's line, between its shoulder and its far end.
    pub fn distance(&self, p: DVec3) -> f64 {
        let along = (p - self.shoulder)
            .dot(self.direction)
            .clamp(0.0, self.length);
        (p - self.at(along)).length()
    }
}

/// Place at → Left arm / Right arm: `shape` wrapped round `arm`, its pattern's "up" (the
/// sleeve cap) towards the shoulder and its length down the arm, the middle of its width at
/// [`SLEEVE_ANGLE`] round it.
/// - It curves round the arm's line, [`PLACE_GAP_M`] clear of the arm where the arm hangs free:
///   the largest surface distance (`surface(along, angle)`, from the arm's line, if a ray finds
///   the arm there) over the angles the piece covers and the stretch of free arm it covers, plus
///   the gap; [`ARM_FALLBACK_RADIUS_M`] when no ray finds the arm there. (Up by the shoulder a
///   ray runs on into the body: a curve that clears that leaves a gap under the arm too wide
///   for the underarm seam to close across.)
/// - Its top goes at the shoulder, or as little further down the arm (in steps of
///   [`ARM_STEP_M`], at most [`ARM_MAX_DROP_M`] or the piece's own length) as keeps every point
///   of it out of the form (`inside(point)`); the cap seams pull it up into the armhole. When
///   no such place is found it stays at the shoulder.
///
/// The placement's own position and turn put that axis on the arm's line, so moving or turning
/// the piece later moves the axis with it, as for any placement.
pub fn place_at_arm(
    shape: &Shape,
    arm: &Arm,
    surface: &dyn Fn(f64, f64) -> Option<f64>,
    inside: &dyn Fn(DVec3) -> bool,
) -> Placement {
    let outline = geom::outline_points(&shape.piece, 0.5);
    let (lo, hi) = crate::bounds(&outline);
    let width = (hi.x - lo.x) / 1000.0;
    let tall = (hi.y - lo.y) / 1000.0;
    let rows = ((tall / ROW_SPACING_M).ceil() as usize).max(1);
    // The radius the piece needs when it is wrapped at radius `r` with its top at `top`.
    let need = |r: f64, top: f64| {
        let half_span = (width / 2.0 / r).min(std::f64::consts::PI);
        let mut farthest: Option<f64> = None;
        for i in 0..=ANGLE_STEPS {
            let angle = SLEEVE_ANGLE - half_span + 2.0 * half_span * i as f64 / ANGLE_STEPS as f64;
            for j in 0..=rows {
                let along = top + tall * j as f64 / rows as f64;
                if along < arm.free {
                    continue;
                }
                if let Some(d) = surface(along, angle).filter(|d| d.is_finite()) {
                    farthest = Some(farthest.map_or(d, |f: f64| f.max(d)));
                }
            }
        }
        farthest
            .map_or(ARM_FALLBACK_RADIUS_M, |d| d + PLACE_GAP_M)
            .clamp(MIN_CURVE_M, MAX_CURVE_M)
    };
    // The placement with the piece's top `top` down the arm and curved to radius `r`.
    let centre = lo.lerp(hi, 0.5);
    let out = arm.around(SLEEVE_ANGLE);
    let up = -arm.direction;
    let turn = DQuat::from_mat3(&DMat3::from_cols(up.cross(out), up, out)).normalize();
    let placed = |top: f64, r: f64| Placement {
        position: (arm.at(top + tall / 2.0) + out * r).to_array(),
        rotation: turn.to_array(),
        curve: Some(r),
    };
    // Its points: along the outline, and a grid inside it, about a spacing apart.
    let spacing = CLEAR_SPACING_MM.max((hi.x - lo.x).max(hi.y - lo.y) / 40.0);
    let mut points = along_outline(&outline, spacing);
    let steps = |a: f64, b: f64| ((b - a) / spacing).ceil() as usize;
    for i in 0..=steps(lo.x, hi.x) {
        for j in 0..=steps(lo.y, hi.y) {
            let q = Point2::new(lo.x + i as f64 * spacing, lo.y + j as f64 * spacing);
            if geom::contains(&shape.piece, q) {
                points.push(q);
            }
        }
    }
    let clear = |p: &Placement| !points.iter().any(|q| inside(apply(p, centre, *q)));
    // At each height, the radius is tried again as for Place at…: the larger of the last two
    // tries covers the span it reaches.
    let at = |top: f64| {
        let (mut previous, mut radius) = (ARM_FALLBACK_RADIUS_M, ARM_FALLBACK_RADIUS_M);
        for _ in 0..RADIUS_TRIES {
            previous = radius;
            radius = need(radius, top);
        }
        placed(top, radius.max(previous))
    };
    let steps = (tall.min(ARM_MAX_DROP_M) / ARM_STEP_M).floor() as usize;
    (0..=steps)
        .map(|k| at(k as f64 * ARM_STEP_M))
        .find(clear)
        .unwrap_or_else(|| at(0.0))
}

/// Points round the closed polyline `outline`, no more than `spacing` apart.
pub fn along_outline(outline: &[Point2], spacing: f64) -> Vec<Point2> {
    let n = outline.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        let steps = (a.distance(b) / spacing).ceil().max(1.0) as usize;
        out.extend((0..steps).map(|i| a.lerp(b, i as f64 / steps as f64)));
    }
    out
}

/// The pattern-table centre of `shape`'s partner in a mirrored pair, if it is in one.
fn partner_centre(project: &Project, shape: &Shape) -> Option<Point2> {
    let partner = match shape.kind {
        ShapeKind::Twin { .. } => shape.source,
        _ => project.piece(shape.source)?.twin.as_ref()?.id,
    };
    geom::shape_of(project, partner).map(|s| centre_of(&s))
}

/// The rotation as angles (degrees) about x, then y, then z, as Properties shows them.
pub fn euler_xyz_deg(rotation: [f64; 4]) -> [f64; 3] {
    let m = DMat3::from_quat(DQuat::from_array(rotation).normalize());
    // m = Rz · Ry · Rx; its element in row r, column c is m.col(c)[r].
    let r20 = m.col(0).z;
    let (x, y, z) = if r20.abs() < 1.0 - 1e-9 {
        (
            m.col(1).z.atan2(m.col(2).z),
            (-r20).asin(),
            m.col(0).y.atan2(m.col(0).x),
        )
    } else {
        // Turned a quarter turn about y: x and z turn about the same axis; keep it all in z.
        let y = if r20 < 0.0 {
            std::f64::consts::FRAC_PI_2
        } else {
            -std::f64::consts::FRAC_PI_2
        };
        (0.0, y, (-m.col(1).x).atan2(m.col(1).y))
    };
    [x, y, z].map(f64::to_degrees)
}

/// The rotation that turns `degrees[0]` about x, then `degrees[1]` about y, then `degrees[2]`
/// about z (the world's axes), stored x, y, z, w.
pub fn rotation_from_euler_xyz_deg(degrees: [f64; 3]) -> [f64; 4] {
    let [x, y, z] = degrees.map(f64::to_radians);
    (DQuat::from_rotation_z(z) * DQuat::from_rotation_y(y) * DQuat::from_rotation_x(x)).to_array()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use opendrape_core::{Piece, PieceId};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn near(a: DVec3, b: DVec3) {
        assert!((a - b).length() < 1e-9, "{a} vs {b}");
    }

    #[test]
    fn a_placement_centres_wraps_turns_and_moves() {
        let flat = Placement::at([0.1, 1.0, 0.4]);
        near(
            apply(&flat, p(500.0, 300.0), p(500.0, 300.0)),
            DVec3::new(0.1, 1.0, 0.4),
        );
        near(
            apply(&flat, p(500.0, 300.0), p(600.0, 250.0)),
            DVec3::new(0.2, 0.95, 0.4),
        );
        let turned = Placement {
            rotation: DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2).to_array(),
            ..flat
        };
        near(
            apply(&turned, p(0.0, 0.0), p(100.0, 0.0)),
            DVec3::new(0.1, 1.0, 0.3),
        );
        // Wrapped round a 0.2 m cylinder whose axis is 0.2 m behind the piece: every point
        // stays on it, the arc keeps the flat length, and the middle doesn't move.
        let r = 0.2;
        let curved = Placement {
            curve: Some(r),
            ..Placement::at([0.0, 0.0, 0.0])
        };
        let axis = DVec3::new(0.0, 0.0, -r);
        for x in [-300.0, -50.0, 0.0, 120.0] {
            let q = apply(&curved, p(0.0, 0.0), p(x, 10.0));
            assert!((DVec2::new(q.x - axis.x, q.z - axis.z).length() - r).abs() < 1e-12);
            let arc = (q.x - axis.x).atan2(q.z - axis.z) * r;
            assert!((arc - x / 1000.0).abs() < 1e-12, "{arc}");
        }
    }

    #[test]
    fn a_mirrored_placement_shows_the_mirror_image() {
        let placement = Placement {
            position: [0.13, 0.8, -0.17],
            rotation: DQuat::from_euler(glam::EulerRot::XYZ, 0.3, 2.1, -0.4).to_array(),
            curve: Some(0.21),
        };
        let m = mirrored(&placement);
        for q in [p(100.0, 50.0), p(-200.0, 300.0), p(0.0, 0.0)] {
            let piece = apply(&placement, p(0.0, 0.0), q);
            let twin = apply(&m, p(0.0, 0.0), p(-q.x, q.y));
            near(twin, DVec3::new(-piece.x, piece.y, piece.z));
        }
        assert!(m.is_valid());
    }

    /// A front (id 1, 0..400 × 0..600), a back (id 2, 600..900 × 0..550) whose twin (id 3)
    /// sits to its right at 1000..1300, all unplaced.
    fn pattern() -> Project {
        let mut pr = Project::new();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            p(0.0, 0.0),
            400.0,
            600.0,
        ));
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(600.0, 0.0),
            300.0,
            550.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), p(1900.0, 0.0))
            .unwrap();
        pr
    }

    #[test]
    fn unplaced_pieces_start_in_front_of_the_form() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        assert_eq!((layout.min, layout.max), (p(0.0, 0.0), p(1300.0, 600.0)));
        // The front's middle (200, 300) is 450 mm left of the layout's middle and 300 mm below
        // its top.
        let front = effective(&pr, &shapes[0], &layout, 1.3);
        assert_eq!(front, Placement::at([-0.45, 1.0, START_DISTANCE_M]));
        // A twin with no placement while its piece has none starts at its own place.
        let twin = effective(&pr, &shapes[2], &layout, 1.3);
        assert!((twin.position[0] - (1150.0 - 650.0) / 1000.0).abs() < 1e-12);
    }

    #[test]
    fn a_twin_mirrors_its_piece_until_it_has_its_own_placement() {
        let mut pr = pattern();
        let placed = Placement {
            position: [0.1, 0.8, -0.2],
            rotation: DQuat::from_rotation_y(2.8).to_array(),
            curve: Some(0.2),
        };
        pr.set_placement(PieceId(2), Some(placed));
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        assert_eq!(effective(&pr, &shapes[2], &layout, 1.3), mirrored(&placed));
        let own = Placement::at([0.0, 0.5, 1.0]);
        pr.set_placement(PieceId(3), Some(own));
        let shapes = geom::shapes(&pr);
        assert_eq!(effective(&pr, &shapes[2], &layout, 1.3), own);
    }

    #[test]
    fn breaking_a_pair_or_deleting_its_piece_does_not_move_the_twin() {
        let twin_id = PieceId(3);
        let at = |pr: &Project| {
            let shapes = geom::shapes(pr);
            let layout = layout(&shapes);
            let shape = shapes.iter().find(|s| s.id == twin_id).unwrap();
            // Where the twin's flat points are in 3D.
            let placement = effective(pr, shape, &layout, 1.3);
            geom::outline_points(&shape.piece, 0.5)
                .into_iter()
                .map(|q| apply(&placement, centre_of(shape), q))
                .collect::<Vec<_>>()
        };
        let mut pr = pattern();
        pr.set_placement(
            PieceId(2),
            Some(Placement {
                position: [0.1, 0.8, -0.2],
                rotation: DQuat::from_rotation_y(2.8).to_array(),
                curve: Some(0.2),
            }),
        );
        let before = at(&pr);
        let mut broken = pr.clone();
        broken.break_twin(PieceId(2));
        let mut deleted = pr.clone();
        deleted.remove_piece(PieceId(2));
        for (what, after) in [("broken off", at(&broken)), ("piece deleted", at(&deleted))] {
            assert_eq!(after.len(), before.len());
            for (a, b) in before.iter().zip(&after) {
                assert!((*a - *b).length() < 1e-9, "{what}: {a} moved to {b}");
            }
        }
    }

    /// A form stand-in: a cylinder of radius 0.15 m round the centre line, 0.5 to 1.5 m up.
    fn cylinder(_angle: f64, y: f64) -> Option<f64> {
        (0.5..=1.5).contains(&y).then_some(0.15)
    }

    #[test]
    fn place_at_wraps_a_piece_round_the_form_at_its_height() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let r = 0.15 + PLACE_GAP_M;
        let height = effective(&pr, &shapes[0], &layout, 1.3).position[1];
        let front = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &cylinder);
        assert_eq!(front.curve, Some(r));
        near(position(&front), DVec3::new(0.0, height, r));
        near(rotation(&front) * DVec3::Z, DVec3::Z);
        let left = place_at(&pr, &shapes[0], PlaceAt::LeftSide, &layout, 1.3, &cylinder);
        near(position(&left), DVec3::new(r, height, 0.0));
        near(rotation(&left) * DVec3::Z, DVec3::X);
        let right = place_at(&pr, &shapes[0], PlaceAt::RightSide, &layout, 1.3, &cylinder);
        near(position(&right), DVec3::new(-r, height, 0.0));
        // Every point of the placed front is the gap clear of the cylinder.
        for x in [0.0, 100.0, 400.0] {
            let q = apply(&front, centre_of(&shapes[0]), p(x, 300.0));
            assert!((DVec2::new(q.x, q.z).length() - r).abs() < 1e-12);
        }
    }

    #[test]
    fn a_pair_meets_at_the_centre_line() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let r = 0.15 + PLACE_GAP_M;
        // The back's right side faces its twin on the table: it goes on the back centre line.
        let back = place_at(&pr, &shapes[1], PlaceAt::Back, &layout, 1.3, &cylinder);
        let corner = apply(&back, centre_of(&shapes[1]), p(900.0, 100.0));
        assert!(
            corner.x.abs() < 1e-12 && (corner.z + r).abs() < 1e-12,
            "{corner}"
        );
        // Its other side is round towards the form's left (+x).
        let other = apply(&back, centre_of(&shapes[1]), p(600.0, 100.0));
        assert!(other.x > 0.0);
        // Its twin, unplaced, mirrors it: the two meet at the centre line.
        let mut placed = pr.clone();
        placed.set_placement(PieceId(2), Some(back));
        let twin = effective(&placed, &geom::shapes(&placed)[2], &layout, 1.3);
        let twin_corner = apply(&twin, centre_of(&shapes[2]), p(1000.0, 100.0));
        near(twin_corner, corner);
    }

    #[test]
    fn a_folded_piece_is_centred_with_its_fold_on_the_centre_line() {
        let mut pr = Project::new();
        let mut half = Piece::rectangle(PieceId(0), "Front", p(100.0, 0.0), 250.0, 550.0);
        half.fold = Some(3); // the left edge, x = 100
        pr.add_piece(half);
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &cylinder);
        let on_fold = apply(&placed, centre_of(&shapes[0]), p(100.0, 200.0));
        assert!(on_fold.x.abs() < 1e-12 && on_fold.z > 0.0, "{on_fold}");
    }

    /// A lone piece, `width` × 600 mm, unplaced (so its height is the starting one, 1.0 m at
    /// shoulders of 1.3 m).
    fn lone(width_mm: f64) -> (Project, PieceId) {
        let mut pr = Project::new();
        let id = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Lone",
            p(0.0, 0.0),
            width_mm,
            600.0,
        ));
        (pr, id)
    }

    /// Whether the placed piece (`width` m wide, `tall` m tall, at `height` m) is clear of the
    /// form everywhere it reaches: across the angles its curve covers (found from the
    /// placement itself) and over its whole height, the form is no farther than the curve's
    /// radius less the gap.
    fn clears(
        surface: &dyn Fn(f64, f64) -> Option<f64>,
        placed: &Placement,
        width: f64,
        tall: f64,
    ) -> Result<(), String> {
        let r = placed.curve.unwrap();
        let phi = placed.position[0].atan2(placed.position[2]);
        let height = placed.position[1];
        for k in 0..=2000 {
            let angle = phi + (-width / 2.0 + width * f64::from(k) / 2000.0) / r;
            for j in 0..=60 {
                let y = height - tall / 2.0 + tall * f64::from(j) / 60.0;
                if let Some(d) = surface(angle, y)
                    && d + PLACE_GAP_M > r + 1e-9
                {
                    return Err(format!(
                        "radius {r:.3} m, but the form is {d:.3} m away at {angle:.3} rad, {y:.2} m up"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn place_at_never_ends_inside_a_step_in_the_form() {
        // An arm beyond 1.4 rad from the front: 0.15 m from the centre line before it, 0.28
        // after. Trying again at the radius it found can swing between the two, and a piece
        // 520-550 mm wide used to stop on the low one, with its edges through the arm.
        let arm = |a: f64, _y: f64| Some(if a.abs() < 1.4 { 0.15 } else { 0.28 });
        // A form that gets farther from the centre line the further round you look.
        let ramp = |a: f64, _y: f64| Some(0.02 + 0.05 * a.abs());
        let (mut arm_radii, mut ramp_radii) = (Vec::new(), Vec::new());
        for width in (300..=700).step_by(10) {
            let (pr, id) = lone(f64::from(width));
            let shapes = geom::shapes(&pr);
            let layout = layout(&shapes);
            for (name, surface, radii) in [
                (
                    "arm",
                    &arm as &dyn Fn(f64, f64) -> Option<f64>,
                    &mut arm_radii,
                ),
                ("ramp", &ramp, &mut ramp_radii),
            ] {
                let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, surface);
                radii.push(placed.curve.unwrap());
                if let Err(why) = clears(surface, &placed, f64::from(width) / 1000.0, 0.6) {
                    panic!("{name}, {width} mm wide ({id:?}): {why}");
                }
            }
        }
        // Both ways of settling are exercised: the arm window needs the high radius.
        assert!(arm_radii.iter().any(|r| (r - 0.31).abs() < 1e-9));
        assert!(arm_radii.iter().any(|r| (r - 0.18).abs() < 1e-9));
        assert!(ramp_radii.windows(2).all(|w| w[1] >= w[0] - 1e-9));
    }

    #[test]
    fn place_at_measures_the_whole_span_and_height_of_the_piece() {
        // A bump in the form 25 cm from the centre line (10 cm elsewhere), over a narrow band
        // of angles right of the middle and the upper part of the piece's height only. A ray
        // down the middle, one at the piece's own height, or one over half the angles misses it.
        let bump = |a: f64, y: f64| {
            Some(if (0.35..0.55).contains(&a) && (1.15..1.25).contains(&y) {
                0.25
            } else {
                0.10
            })
        };
        let (pr, _) = lone(500.0);
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &bump);
        assert_eq!(placed.curve, Some(0.25 + PLACE_GAP_M));
        assert_eq!(clears(&bump, &placed, 0.5, 0.6), Ok(()));
        // The same far form on the other side (angles are positive towards +x).
        let mirror = |a: f64, y: f64| bump(-a, y);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &mirror);
        assert_eq!(placed.curve, Some(0.25 + PLACE_GAP_M));
        // And below the piece's reach: nothing.
        let low = |a: f64, y: f64| bump(a, y + 0.9);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &low);
        assert_eq!(placed.curve, Some(0.10 + PLACE_GAP_M));
    }

    #[test]
    fn a_member_of_a_pair_is_measured_across_the_side_it_curves_round() {
        // The back's right edge (towards its twin) goes on the back centre line, so the piece
        // curves round towards the form's left: angles from π less its width / radius up to π.
        // A bump there, and only there, sets its radius.
        let bump = |a: f64, _y: f64| {
            let round = std::f64::consts::PI;
            Some(if (round - 1.0..round - 0.8).contains(&a) {
                0.25
            } else {
                0.10
            })
        };
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let back = place_at(&pr, &shapes[1], PlaceAt::Back, &layout, 1.3, &bump);
        assert_eq!(clears(&bump, &back, 0.3, 0.55), Ok(()));
        assert!(
            back.curve.unwrap() >= 0.25 + PLACE_GAP_M,
            "{:?}",
            back.curve
        );
        // Placed at the front, the same piece curves round towards the form's right instead
        // (its facing side goes to angle 0 and it covers 0..-w/r): the bump is not in reach.
        let front = place_at(&pr, &shapes[1], PlaceAt::Front, &layout, 1.3, &bump);
        assert_eq!(front.curve, Some(0.10 + PLACE_GAP_M));
    }

    #[test]
    fn with_no_form_in_reach_place_at_uses_a_fallback_curve() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &|_, _| None);
        assert_eq!(placed.curve, Some(FALLBACK_RADIUS_M));
        assert!(placed.is_valid());
    }

    #[test]
    fn typed_angles_turn_about_x_then_y_then_z() {
        let q = DQuat::from_array(rotation_from_euler_xyz_deg([90.0, 90.0, 0.0]));
        near(q * DVec3::Y, DVec3::X); // y turned about x is z; z turned about y is x
        for degrees in [
            [10.0, 20.0, 30.0],
            [-45.0, 80.0, 170.0],
            [0.0, 0.0, -90.0],
            [0.0, 90.0, 0.0],
            [30.0, -90.0, 10.0],
        ] {
            let q = rotation_from_euler_xyz_deg(degrees);
            let back = rotation_from_euler_xyz_deg(euler_xyz_deg(q));
            let dot: f64 = q.iter().zip(&back).map(|(a, b)| a * b).sum();
            assert!(
                dot.abs() > 1.0 - 1e-9,
                "{degrees:?} → {:?}",
                euler_xyz_deg(q)
            );
        }
        let e = euler_xyz_deg(rotation_from_euler_xyz_deg([10.0, 20.0, 30.0]));
        for (a, b) in e.iter().zip([10.0, 20.0, 30.0]) {
            assert!((a - b).abs() < 1e-9, "{e:?}");
        }
    }

    /// An arm stand-in on the form's left: from (0.18, 1.3, 0) down and out, a little forward.
    fn left_arm() -> Arm {
        Arm {
            shoulder: DVec3::new(0.18, 1.3, 0.0),
            direction: DVec3::new(0.5, -0.85, 0.1).normalize(),
            length: 0.6,
            free: 0.0,
        }
    }

    /// A sleeve: 340 mm wide, 220 mm long, with its twin.
    fn sleeves() -> Project {
        let mut pr = Project::new();
        let id = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            p(0.0, 0.0),
            340.0,
            220.0,
        ));
        pr.add_twin(id, "Sleeve (mirror)".into(), p(800.0, 0.0))
            .unwrap();
        pr
    }

    #[test]
    fn an_arm_has_a_front_and_an_outer_side_the_same_way_round_on_both() {
        let left = left_arm();
        let right = Arm {
            shoulder: left.shoulder * DVec3::new(-1.0, 1.0, 1.0),
            direction: left.direction * DVec3::new(-1.0, 1.0, 1.0),
            ..left
        };
        for arm in [left, right] {
            let (front, out) = (arm.around(0.0), arm.around(std::f64::consts::FRAC_PI_2));
            assert!(front.z > 0.9, "the front faces forward: {front}");
            assert!(
                out.x * arm.direction.x > 0.0 && out.y > 0.0,
                "out and up: {out}"
            );
            for v in [front, out] {
                assert!(v.dot(arm.direction).abs() < 1e-12 && (v.length() - 1.0).abs() < 1e-12);
            }
        }
        let mirror = |v: DVec3| v * DVec3::new(-1.0, 1.0, 1.0);
        for angle in [0.0, 0.7, 2.0, -1.2] {
            near(right.around(angle), mirror(left.around(angle)));
        }
        // 5 cm in front of the shoulder; and past the far end, measured from the end.
        assert!((left.distance(left.shoulder + left.around(0.0) * 0.05) - 0.05).abs() < 1e-12);
        assert!((left.distance(left.at(0.7)) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn place_at_arm_wraps_a_sleeve_round_the_arm_line_with_its_top_at_the_shoulder() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let arm = left_arm();
        // An arm 4.5 cm thick all the way down.
        let thick = |along: f64, _angle: f64| (0.0..=0.6).contains(&along).then_some(0.045);
        let placed = place_at_arm(&shapes[0], &arm, &thick, &|_| false);
        let r = 0.045 + PLACE_GAP_M;
        assert_eq!(placed.curve, Some(r));
        assert!(placed.is_valid());
        let centre = centre_of(&shapes[0]);
        for x in [0.0, 85.0, 170.0, 300.0, 340.0] {
            for y in [0.0, 110.0, 220.0] {
                let q = apply(&placed, centre, p(x, y));
                let along = (q - arm.shoulder).dot(arm.direction);
                assert!(
                    (arm.distance(q) - r).abs() < 1e-9,
                    "({x}, {y}) is round the arm"
                );
                // The top of the sleeve is level with the shoulder, its hem 220 mm down.
                assert!(
                    (along - (220.0 - y) / 1000.0).abs() < 1e-9,
                    "({x}, {y}): {along}"
                );
            }
        }
        // The middle of its width is on the arm's outer side.
        let middle = apply(&placed, centre, p(170.0, 110.0));
        near(middle, arm.at(0.11) + arm.around(SLEEVE_ANGLE) * r);
        // Its left half (on the pattern) is towards the front of the arm.
        assert!(apply(&placed, centre, p(85.0, 110.0)).z > middle.z);
    }

    #[test]
    fn the_twin_of_a_sleeve_on_one_arm_is_on_the_other() {
        let mut pr = sleeves();
        let arm = left_arm();
        let shapes = geom::shapes(&pr);
        let placed = place_at_arm(&shapes[0], &arm, &|_, _| Some(0.045), &|_| false);
        pr.set_placement(PieceId(1), Some(placed));
        let shapes = geom::shapes(&pr);
        let twin = effective(&pr, &shapes[1], &layout(&shapes), 1.3);
        let right = Arm {
            shoulder: arm.shoulder * DVec3::new(-1.0, 1.0, 1.0),
            direction: arm.direction * DVec3::new(-1.0, 1.0, 1.0),
            ..arm
        };
        for q in geom::outline_points(&shapes[1].piece, 1.0) {
            let at = apply(&twin, centre_of(&shapes[1]), q);
            assert!((right.distance(at) - 0.075).abs() < 1e-9, "{at}");
        }
    }

    #[test]
    fn a_sleeve_moved_after_placing_takes_its_curve_with_it() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let arm = left_arm();
        let placed = place_at_arm(&shapes[0], &arm, &|_, _| Some(0.045), &|_| false);
        // Moved 5 cm forward and turned 10° about y, as with the gizmo: still wrapped round
        // the arm's line moved and turned the same way, not round the body's centre line.
        let turn = DQuat::from_rotation_y(10f64.to_radians());
        let shift = DVec3::new(0.0, 0.0, 0.05);
        let centre_before = position(&placed);
        let moved = Placement {
            position: (centre_before + shift).to_array(),
            rotation: (turn * rotation(&placed)).to_array(),
            ..placed
        };
        let moved_arm = Arm {
            shoulder: centre_before + shift + turn * (arm.shoulder - centre_before),
            direction: turn * arm.direction,
            ..arm
        };
        for q in geom::outline_points(&shapes[0].piece, 1.0) {
            let at = apply(&moved, centre_of(&shapes[0]), q);
            assert!((moved_arm.distance(at) - 0.075).abs() < 1e-9, "{at}");
        }
    }

    #[test]
    fn with_no_arm_in_reach_place_at_arm_uses_a_fallback_curve() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let placed = place_at_arm(&shapes[0], &left_arm(), &|_, _| None, &|_| false);
        assert_eq!(placed.curve, Some(ARM_FALLBACK_RADIUS_M));
        assert!(placed.is_valid());
    }

    #[test]
    fn place_at_arm_measures_the_arm_only_where_it_hangs_free() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        // Down to 10 cm the arm's rays run into the body, 15 cm out; below, it is 4.5 cm thick.
        let arm = Arm {
            free: 0.1,
            ..left_arm()
        };
        let joined = |along: f64, _: f64| Some(if along < 0.1 { 0.15 } else { 0.045 });
        let placed = place_at_arm(&shapes[0], &arm, &joined, &|_| false);
        assert_eq!(placed.curve, Some(0.045 + PLACE_GAP_M));
    }

    #[test]
    fn place_at_arm_moves_a_piece_down_the_arm_until_it_is_clear_of_the_form() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let arm = left_arm();
        // A form whose shoulder fills everything above 1.27 m.
        let shoulder = |q: DVec3| q.y > 1.27;
        let surface = |_: f64, _: f64| Some(0.045);
        let placed = place_at_arm(&shapes[0], &arm, &surface, &shoulder);
        let centre = centre_of(&shapes[0]);
        let outline = along_outline(&geom::outline_points(&shapes[0].piece, 0.5), 1.0);
        let highest = |p: &Placement| {
            outline
                .iter()
                .map(|q| apply(p, centre, *q).y)
                .fold(f64::MIN, f64::max)
        };
        assert!(highest(&placed) <= 1.27, "clear: {}", highest(&placed));
        // And no further down than it had to go: a centimetre higher, it is not clear.
        let higher = Placement {
            position: (position(&placed) - arm.direction * 0.01).to_array(),
            ..placed
        };
        assert!(highest(&higher) > 1.27, "{}", highest(&higher));
        // Still wrapped round the arm, its top somewhere down from the shoulder.
        let top = (position(&placed) - arm.shoulder).dot(arm.direction) - 0.11;
        assert!(top > 0.0 && top <= 0.22, "{top}");
        // With no clear place on the arm at all, it stays at the shoulder.
        let everywhere = place_at_arm(&shapes[0], &arm, &surface, &|_| true);
        near(
            position(&everywhere),
            arm.at(0.11) + arm.around(SLEEVE_ANGLE) * 0.075,
        );
    }

    #[test]
    fn place_at_arm_on_a_huge_piece_checks_a_bounded_number_of_points() {
        // A 2 m square (no sleeve; chosen by mistake), on an arm where nothing is ever clear:
        // the search gives up after 20 cm, checking at most a few thousand points each step.
        let mut pr = Project::new();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Huge",
            p(0.0, 0.0),
            2000.0,
            2000.0,
        ));
        let shapes = geom::shapes(&pr);
        let checked = std::cell::Cell::new(0_usize);
        let placed = place_at_arm(&shapes[0], &left_arm(), &|_, _| Some(0.045), &|_| {
            checked.set(checked.get() + 1);
            true
        });
        assert!(placed.is_valid());
        assert!(
            checked.get() < 21 * 2_000,
            "{} points checked",
            checked.get()
        );
        // It stays at the shoulder: its top level with it.
        let top = (position(&placed) - left_arm().shoulder).dot(left_arm().direction) - 1.0;
        assert!(top.abs() < 1e-9, "{top}");
    }
}
