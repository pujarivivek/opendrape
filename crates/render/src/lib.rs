//! OpenDrape 3D rendering. Scenes are drawn into offscreen textures that the app
//! shows in its 3D panel, and that tests (and later PNG export) read back.

mod camera;
mod cube;
mod headless;
mod target;

pub use camera::OrbitCamera;
pub use cube::CubeRenderer;
pub use headless::{HeadlessGpu, headless_device, headless_device_with, read_back};
pub use target::{CLEAR_COLOR, COLOR_FORMAT, DEPTH_FORMAT, RenderTarget, target_size};
