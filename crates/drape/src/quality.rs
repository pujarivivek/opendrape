//! How finely the fabric is made and simulated: detail traded for speed, for the laptops
//! students have. A coarser lattice means fewer particles, which is what a frame costs.

use opendrape_mesh::MeshParams;
use opendrape_sim::Params;

/// A fabric detail preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrapeQuality {
    /// 20 mm cells, cloth against cloth every other substep: quickest.
    Draft,
    /// 12 mm cells.
    #[default]
    Normal,
    /// 8 mm cells: small folds, at about twice Normal's cost.
    Fine,
}

impl DrapeQuality {
    pub const ALL: [Self; 3] = [Self::Draft, Self::Normal, Self::Fine];

    /// The lattice spacing (mm).
    pub fn edge_mm(self) -> f64 {
        match self {
            Self::Draft => 20.0,
            Self::Normal => 12.0,
            Self::Fine => 8.0,
        }
    }

    pub fn mesh_params(self) -> MeshParams {
        MeshParams {
            edge_mm: self.edge_mm(),
            ..MeshParams::default()
        }
    }

    /// One pass per substep: on fabric with a bias, 30 substeps of one pass give less strain
    /// than the solver's default 20 of two at three quarters of the link solving, and 24 of
    /// one the same strain at just over half (the sweep in docs/testing/bench.md).
    pub fn params(self) -> Params {
        let play = Params::default();
        match self {
            Self::Draft => Params {
                substeps: 20,
                iterations: 1,
                self_collision_every: 2,
                ..play
            },
            Self::Normal | Self::Fine => Params {
                substeps: 30,
                iterations: 1,
                ..play
            },
        }
    }

    /// The preset for a computer with `cores` logical cores: Draft on two or fewer, where
    /// the solver shares its one spare core with everything else; Normal otherwise.
    pub fn for_cores(cores: usize) -> Self {
        if cores <= 2 {
            Self::Draft
        } else {
            Self::Normal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_trade_cells_for_speed_and_a_small_laptop_gets_draft() {
        assert!(DrapeQuality::Draft.edge_mm() > DrapeQuality::Normal.edge_mm());
        assert!(DrapeQuality::Normal.edge_mm() > DrapeQuality::Fine.edge_mm());
        assert_eq!(DrapeQuality::Normal.mesh_params(), MeshParams::default());
        let (normal, draft) = (DrapeQuality::Normal.params(), DrapeQuality::Draft.params());
        assert_eq!((normal.substeps, normal.iterations), (30, 1));
        assert_eq!((draft.substeps, draft.iterations), (20, 1));
        assert_eq!(draft.self_collision_every, 2);
        assert_eq!(normal.self_collision_every, 1);
        assert_eq!(DrapeQuality::Fine.params(), normal);
        assert_eq!(DrapeQuality::for_cores(1), DrapeQuality::Draft);
        assert_eq!(DrapeQuality::for_cores(2), DrapeQuality::Draft);
        assert_eq!(DrapeQuality::for_cores(4), DrapeQuality::Normal);
        assert_eq!(DrapeQuality::default(), DrapeQuality::Normal);
    }
}
