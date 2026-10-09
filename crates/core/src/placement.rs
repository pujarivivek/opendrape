//! Where a piece sits in 3D. Placements are in the form's frame: metres, y up from the floor,
//! the form faces +z and its left is +x, and x = 0, z = 0 is its centre line.

use serde::{Deserialize, Serialize};

/// The tightest curve a piece may be wrapped round (m).
pub const MIN_CURVE_M: f64 = 0.05;
/// The widest curve a piece may be wrapped round (m).
pub const MAX_CURVE_M: f64 = 2.0;
/// The furthest from the origin a piece may be placed (m).
pub const MAX_PLACEMENT_M: f64 = 10.0;

/// A piece's place in 3D. Applying it to the flat piece: centre the piece on the middle of its
/// bounding box; wrap it round a vertical cylinder of radius `curve` whose axis lies behind it
/// (when `curve` is set); turn it by `rotation`; move it to `position`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// Where the middle of the piece goes (m).
    pub position: [f64; 3],
    /// A unit quaternion, stored x, y, z, w.
    pub rotation: [f64; 4],
    /// The radius (m) of the cylinder the piece is wrapped round; None for a flat piece.
    #[serde(default)]
    pub curve: Option<f64>,
}

impl Placement {
    /// No turn at all.
    pub const NO_ROTATION: [f64; 4] = [0.0, 0.0, 0.0, 1.0];

    /// Flat and unturned, with its middle at `position`.
    pub fn at(position: [f64; 3]) -> Self {
        Self {
            position,
            rotation: Self::NO_ROTATION,
            curve: None,
        }
    }

    /// Every number finite; the rotation of unit length (within 1e-6); the curve, if any,
    /// between [`MIN_CURVE_M`] and [`MAX_CURVE_M`]; the position within [`MAX_PLACEMENT_M`] of
    /// the origin.
    pub fn is_valid(&self) -> bool {
        let finite = self
            .position
            .iter()
            .chain(&self.rotation)
            .all(|v| v.is_finite());
        let length = self.rotation.iter().map(|v| v * v).sum::<f64>().sqrt();
        let distance = self.position.iter().map(|v| v * v).sum::<f64>().sqrt();
        finite
            && (length - 1.0).abs() <= 1e-6
            && distance <= MAX_PLACEMENT_M
            && self
                .curve
                .is_none_or(|r| r.is_finite() && (MIN_CURVE_M..=MAX_CURVE_M).contains(&r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_are_checked() {
        let good = Placement {
            position: [0.1, 1.0, 0.3],
            rotation: [0.0, 0.6, 0.0, 0.8],
            curve: Some(0.2),
        };
        assert!(good.is_valid() && Placement::at([0.0, 0.0, 0.0]).is_valid());
        let bad = [
            Placement {
                position: [f64::NAN, 0.0, 0.0],
                ..good
            },
            Placement {
                rotation: [0.0, 0.0, 0.0, 1.001],
                ..good
            },
            Placement {
                rotation: [0.0; 4],
                ..good
            },
            Placement {
                rotation: [0.0, 0.0, f64::INFINITY, 1.0],
                ..good
            },
            Placement {
                curve: Some(0.049),
                ..good
            },
            Placement {
                curve: Some(2.01),
                ..good
            },
            Placement {
                curve: Some(f64::NAN),
                ..good
            },
            Placement {
                position: [8.0, 0.0, 6.1],
                ..good
            },
        ];
        for p in bad {
            assert!(!p.is_valid(), "{p:?}");
        }
        assert!(
            Placement {
                rotation: [0.0, 0.0, 0.0, 1.0 + 9e-7],
                ..good
            }
            .is_valid()
        );
    }

    #[test]
    fn a_flat_placement_saves_without_a_curve() {
        let json = r#"{"position":[0.0,1.2,0.4],"rotation":[0.0,0.0,0.0,1.0]}"#;
        let p: Placement = serde_json::from_str(json).unwrap();
        assert_eq!(p, Placement::at([0.0, 1.2, 0.4]));
    }
}
