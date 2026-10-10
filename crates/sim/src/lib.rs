//! XPBD cloth simulation on the CPU in f64: fabric stretch and bending, seams that pull shut
//! and then weld, and collision against a static body.

mod attach;
mod cloth;
mod collide;
mod solver;

pub use attach::AttachmentId;
pub use cloth::{Cloth, ClothBuilder, Panel, PanelId};
pub use collide::{BodyCollider, Collider, ColliderError, CompoundCollider, Plane, Solid};
pub use solver::{FRAME_DT, Params, Solver};
