//! Where a frame's time goes, phase by phase, so a change to the solver can be measured
//! rather than guessed at. The clock reads cost tens of nanoseconds each, a few hundred per
//! frame: nothing beside the millions of constraint solves they time.

use std::time::Instant;

/// Milliseconds the last frame spent in each phase of [`crate::Solver::step`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhaseTimes {
    /// Merging closed seams into continuous fabric.
    pub weld: f64,
    /// Finding each particle's contact plane on the body.
    pub body_query: f64,
    /// Gravity, damping and the predicted positions.
    pub predict: f64,
    pub stitches: f64,
    pub bend: f64,
    pub stretch: f64,
    /// Pins and grabs.
    pub attach: f64,
    /// Cloth against cloth.
    pub self_collide: f64,
    /// Pushing particles out of the body and the floor, with friction.
    pub collide: f64,
    /// Velocities from the positions.
    pub velocity: f64,
}

impl PhaseTimes {
    pub const NAMES: [&'static str; 10] = [
        "weld",
        "body query",
        "predict",
        "stitches",
        "bend",
        "stretch",
        "attach",
        "self-collide",
        "collide",
        "velocity",
    ];

    /// The phases in the order of [`Self::NAMES`].
    pub fn values(&self) -> [f64; 10] {
        [
            self.weld,
            self.body_query,
            self.predict,
            self.stitches,
            self.bend,
            self.stretch,
            self.attach,
            self.self_collide,
            self.collide,
            self.velocity,
        ]
    }

    pub fn total(&self) -> f64 {
        self.values().iter().sum()
    }

    /// Adds `other` phase by phase (for averaging over frames).
    pub fn add(&mut self, other: &PhaseTimes) {
        self.weld += other.weld;
        self.body_query += other.body_query;
        self.predict += other.predict;
        self.stitches += other.stitches;
        self.bend += other.bend;
        self.stretch += other.stretch;
        self.attach += other.attach;
        self.self_collide += other.self_collide;
        self.collide += other.collide;
        self.velocity += other.velocity;
    }

    pub fn scaled(&self, by: f64) -> PhaseTimes {
        PhaseTimes {
            weld: self.weld * by,
            body_query: self.body_query * by,
            predict: self.predict * by,
            stitches: self.stitches * by,
            bend: self.bend * by,
            stretch: self.stretch * by,
            attach: self.attach * by,
            self_collide: self.self_collide * by,
            collide: self.collide * by,
            velocity: self.velocity * by,
        }
    }
}

/// A stopwatch that hands each lap's milliseconds to a phase's total.
pub(crate) struct Lap(Instant);

impl Lap {
    pub(crate) fn start() -> Self {
        Self(Instant::now())
    }

    pub(crate) fn lap(&mut self, into: &mut f64) {
        let now = Instant::now();
        *into += (now - self.0).as_secs_f64() * 1000.0;
        self.0 = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_and_averages_add_up() {
        let mut t = PhaseTimes {
            bend: 1.0,
            stretch: 2.0,
            ..Default::default()
        };
        assert_eq!(t.total(), 3.0);
        let same = t;
        t.add(&same);
        assert_eq!(t.scaled(0.5).stretch, 2.0);
        assert_eq!(PhaseTimes::NAMES.len(), t.values().len());
    }

    #[test]
    fn a_lap_hands_its_time_on_and_restarts() {
        let mut l = Lap::start();
        let mut ms = 0.0;
        l.lap(&mut ms);
        assert!(ms >= 0.0);
        let again = l.0;
        l.lap(&mut ms);
        assert!(l.0 >= again);
    }
}
