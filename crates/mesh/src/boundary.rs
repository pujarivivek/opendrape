//! The points round one shape's outline that become the fabric's edge: every corner; along each
//! seam side, the side's own samples (so both sides of a seam have the same number); and along
//! the stretches no side covers, points about `h` apart. Two sides that meet part-way along an
//! edge share the point where they meet.

use opendrape_core::{Point2, SeamSide};
use opendrape_geom::{self as geom, Run, Shape};

/// A shape's outline as points (mm), and where things are among them.
pub(crate) struct Outline {
    pub points: Vec<Point2>,
    /// For each outline edge: its start corner, the points along it, its end corner.
    pub edges: Vec<Vec<u32>>,
    /// For each side asked for: the point of each of its samples, from its start.
    pub sides: Vec<Vec<u32>>,
}

/// A point along an outline edge: its distance (mm) from the edge's start, and the side
/// sample it is (side, sample), if any.
type Along = (f64, Option<(usize, usize)>);

/// The outline of `shape` with each of `sides` (a side on this shape, and its samples as
/// distances along it from its start) in place. None when a side does not fit the shape.
pub(crate) fn outline(shape: &Shape, sides: &[(SeamSide, Vec<f64>)], h: f64) -> Option<Outline> {
    let piece = &shape.piece;
    let m = piece.len();
    let lens: Vec<f64> = (0..m).map(|j| geom::edge_length(piece, j)).collect();
    let tiny = |j: usize| 1e-9 * lens[j].max(1.0);
    // Per outline edge, the stretches sides cover, and the points along it.
    let mut covered: Vec<Vec<(f64, f64)>> = vec![Vec::new(); m];
    let mut along: Vec<Vec<Along>> = vec![Vec::new(); m];
    // Side samples that land on a corner: (side, sample, corner).
    let mut on_corner = Vec::new();
    for (s, (side, at)) in sides.iter().enumerate() {
        let runs = geom::side_runs(shape, side)?;
        for r in &runs {
            covered[r.edge].push((r.from.min(r.to), r.from.max(r.to)));
        }
        for (k, d) in at.iter().enumerate() {
            let (edge, x) = locate(&runs, *d);
            if x <= tiny(edge) {
                on_corner.push((s, k, edge));
            } else if x >= lens[edge] - tiny(edge) {
                on_corner.push((s, k, (edge + 1) % m));
            } else {
                along[edge].push((x, Some((s, k))));
            }
        }
    }
    for j in 0..m {
        for (g0, g1) in gaps(&mut covered[j], lens[j], tiny(j)) {
            let steps = (((g1 - g0) / h).round() as usize).max(1);
            along[j].extend((1..steps).map(|q| (g0 + (g1 - g0) * q as f64 / steps as f64, None)));
        }
        along[j].sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let mut points = Vec::new();
    let mut corners = Vec::with_capacity(m);
    let mut inner: Vec<Vec<u32>> = vec![Vec::new(); m];
    let mut side_points: Vec<Vec<u32>> = sides.iter().map(|(_, at)| vec![0; at.len()]).collect();
    for j in 0..m {
        corners.push(points.len() as u32);
        points.push(piece.vertices[j].pos);
        // Points at the same place (where two sides meet) are one point.
        let mut last: Option<(f64, u32)> = None;
        for &(d, sample) in &along[j] {
            let index = match last {
                Some((at, index)) if d - at <= tiny(j) => index,
                _ => {
                    let index = points.len() as u32;
                    points.push(geom::point_at_distance(piece, j, d));
                    inner[j].push(index);
                    index
                }
            };
            last = Some((d, index));
            if let Some((s, k)) = sample {
                side_points[s][k] = index;
            }
        }
    }
    for (s, k, corner) in on_corner {
        side_points[s][k] = corners[corner];
    }
    let edges = (0..m)
        .map(|j| {
            let mut e = vec![corners[j]];
            e.extend(&inner[j]);
            e.push(corners[(j + 1) % m]);
            e
        })
        .collect();
    Some(Outline {
        points,
        edges,
        sides: side_points,
    })
}

/// Where distance `d` along a side made of `runs` is: the outline edge, and how far along it
/// from its start. A distance at the end of one run is the start of the next (the same corner).
fn locate(runs: &[Run], d: f64) -> (usize, f64) {
    let mut start = 0.0;
    let mut r = 0;
    while r + 1 < runs.len() && d >= start + runs[r].length() {
        start += runs[r].length();
        r += 1;
    }
    let run = runs[r];
    let local = (d - start).clamp(0.0, run.length());
    let x = if run.to >= run.from {
        run.from + local
    } else {
        run.from - local
    };
    (run.edge, x)
}

/// The stretches of an edge `len` mm long that none of `covered` covers, longer than `tiny`.
fn gaps(covered: &mut [(f64, f64)], len: f64, tiny: f64) -> Vec<(f64, f64)> {
    covered.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut at = 0.0;
    for &(lo, hi) in covered.iter() {
        if lo - at > tiny {
            out.push((at, lo));
        }
        at = f64::max(at, hi);
    }
    if len - at > tiny {
        out.push((at, len));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stretches_nobody_sews_are_what_is_left_of_the_edge() {
        let mut covered = vec![(60.0, 80.0), (10.0, 30.0), (30.0, 40.0)];
        assert_eq!(
            gaps(&mut covered, 100.0, 1e-9),
            vec![(0.0, 10.0), (40.0, 60.0), (80.0, 100.0)]
        );
        assert_eq!(gaps(&mut [(0.0, 100.0)], 100.0, 1e-9), vec![]);
        assert_eq!(gaps(&mut [], 100.0, 1e-9), vec![(0.0, 100.0)]);
    }

    #[test]
    fn a_distance_along_a_side_is_found_on_its_runs() {
        let runs = [
            Run {
                edge: 1,
                from: 30.0,
                to: 100.0,
            },
            Run {
                edge: 2,
                from: 50.0,
                to: 0.0,
            },
        ];
        assert_eq!(locate(&runs, 0.0), (1, 30.0));
        assert_eq!(locate(&runs, 69.0), (1, 99.0));
        assert_eq!(
            locate(&runs, 70.0),
            (2, 50.0),
            "the corner, on the next run"
        );
        assert_eq!(locate(&runs, 100.0), (2, 20.0), "running backwards");
        assert_eq!(locate(&runs, 120.0), (2, 0.0));
    }
}
