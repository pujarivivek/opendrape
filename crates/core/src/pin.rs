//! Pins: a spot of fabric held at a point in 3D while it drapes. A pin is stored by where it is
//! on the pattern, not by a point of the fabric mesh, so it outlives any change to the mesh.

use crate::{Half, PieceId, Point2};
use serde::{Deserialize, Serialize};

/// Most pins a project may hold.
pub const MAX_PINS: usize = 500;

/// How far (mm) outside its piece's outline a pin may be and still count as on it.
pub const PIN_SLACK_MM: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    /// The piece or twin it is on.
    pub shape: PieceId,
    /// The half of a cut-on-fold piece it is on; `Drawn` for every other shape.
    #[serde(default)]
    pub half: Half,
    /// Where it is on the stored piece (mm): for a twin, where its piece shows that spot; on
    /// the pale half of a fold, the mirror image across the fold of the spot pinned.
    pub at: Point2,
    /// Where that spot of fabric is held (m, in the form's frame).
    pub target: [f64; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_serialise_plainly() {
        let pin = Pin {
            shape: PieceId(3),
            half: Half::Pale,
            at: Point2::new(12.5, 40.0),
            target: [0.1, 1.2, -0.05],
        };
        let json = serde_json::to_string(&pin).unwrap();
        assert_eq!(
            json,
            r#"{"shape":3,"half":"pale","at":{"x":12.5,"y":40.0},"target":[0.1,1.2,-0.05]}"#
        );
        assert_eq!(serde_json::from_str::<Pin>(&json).unwrap(), pin);
        let drawn = r#"{"shape":3,"at":{"x":12.5,"y":40.0},"target":[0.1,1.2,-0.05]}"#;
        assert_eq!(
            serde_json::from_str::<Pin>(drawn).unwrap().half,
            Half::Drawn
        );
    }
}
