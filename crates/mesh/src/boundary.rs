//! The points round one shape's outline that become the fabric's edge: every corner, points
//! about `h` apart along free edges, and along each seam side the side's own count of points
//! at equal steps, so both sides of a seam have the same number.

use opendrape_core::{Point2, SeamSide};
use opendrape_geom::{self as geom, Shape};

/// A shape's outline as points (mm), and where things are among them.
pub(crate) struct Outline {
    pub points: Vec<Point2>,
    /// For each outline edge: its start corner, the points along it, its end corner.
    pub edges: Vec<Vec<u32>>,
    /// For each side asked for: the point of each of its `steps + 1` samples, from its start.
    pub sides: Vec<Vec<u32>>,
}

/// A point along an outline edge: its distance (mm) from the edge's start, and the side
/// sample it is (side, sample), if any.
type Along = (f64, Option<(usize, usize)>);

/// One sample of a side: on outline edge `edge`, `along` mm from that edge's start.
#[derive(Clone, Copy, Debug)]
struct At {
    edge: usize,
    along: f64,
}

/// The outline of `shape` with each of `sides` (a side on this shape, and its step count)
/// sampled at equal steps. None when a side does not fit the shape.
pub(crate) fn outline(shape: &Shape, sides: &[(SeamSide, usize)], h: f64) -> Option<Outline> {
    let piece = &shape.piece;
    let m = piece.len();
    let lens: Vec<f64> = (0..m).map(|j| geom::edge_length(piece, j)).collect();
    let mut sewn = vec![false; m];
    // Per outline edge, the points along it.
    let mut along: Vec<Vec<Along>> = vec![Vec::new(); m];
    // Side samples that land on a corner: (side, sample, corner).
    let mut on_corner = Vec::new();
    for (s, (side, steps)) in sides.iter().enumerate() {
        let runs = geom::side_edges(shape, side)?;
        for (j, _) in &runs {
            sewn[*j] = true;
        }
        for (k, at) in side_samples(&runs, &lens, *steps).into_iter().enumerate() {
            let len = lens[at.edge];
            let tiny = 1e-9 * len.max(1.0);
            if at.along <= tiny {
                on_corner.push((s, k, at.edge));
            } else if at.along >= len - tiny {
                on_corner.push((s, k, (at.edge + 1) % m));
            } else {
                along[at.edge].push((at.along, Some((s, k))));
            }
        }
    }
    for j in 0..m {
        if !sewn[j] {
            let steps = ((lens[j] / h).round() as usize).max(1);
            along[j].extend((1..steps).map(|q| (lens[j] * q as f64 / steps as f64, None)));
        }
        along[j].sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let mut points = Vec::new();
    let mut corners = Vec::with_capacity(m);
    let mut inner: Vec<Vec<u32>> = vec![Vec::new(); m];
    let mut side_points: Vec<Vec<u32>> = sides.iter().map(|(_, n)| vec![0; n + 1]).collect();
    for j in 0..m {
        corners.push(points.len() as u32);
        points.push(piece.vertices[j].pos);
        for &(d, sample) in &along[j] {
            let index = points.len() as u32;
            points.push(geom::point_at_distance(piece, j, d));
            inner[j].push(index);
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

/// The `steps + 1` samples of a side made of `runs` (outline edge, and whether the side runs
/// against it), at equal steps of arc length from the side's start. Each corner inside the
/// side takes over the sample nearest to it, so no sample lies a sliver away from a corner;
/// a corner whose nearest sample another corner took keeps no sample.
fn side_samples(runs: &[(usize, bool)], lens: &[f64], steps: usize) -> Vec<At> {
    let total: f64 = runs.iter().map(|(j, _)| lens[*j]).sum();
    let step = total / steps as f64;
    let mut at: Vec<f64> = (0..=steps).map(|k| k as f64 * step).collect();
    at[steps] = total;
    let mut taken = vec![false; steps + 1];
    let mut start = 0.0;
    for (j, _) in &runs[..runs.len() - 1] {
        start += lens[*j];
        if steps >= 2 {
            let k = ((start / step).round() as usize).clamp(1, steps - 1);
            if !taken[k] {
                taken[k] = true;
                at[k] = start;
            }
        }
    }
    at.into_iter()
        .map(|s| {
            // The run this sample is on: the last one starting at or before it.
            let mut start = 0.0;
            let mut r = 0;
            while r + 1 < runs.len() && s >= start + lens[runs[r].0] {
                start += lens[runs[r].0];
                r += 1;
            }
            let (edge, against) = runs[r];
            let local = (s - start).clamp(0.0, lens[edge]);
            At {
                edge,
                along: if against { lens[edge] - local } else { local },
            }
        })
        .collect()
}
