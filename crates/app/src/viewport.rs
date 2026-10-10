use crate::arrange::{ArrangedScene, ScreenCamera};
use crate::sim_runner::SimFrame;
use glam::DVec2;
use opendrape_core::PieceId;
use opendrape_drape::Stage;
use opendrape_render::{GpuMesh, MeshRenderer, OrbitCamera, RenderTarget, target_size};
use std::rc::Rc;
use std::sync::Arc;

/// Mid-brown skin tone, a cotton blue, and the selected piece's warmer blue.
const SKIN: [f32; 3] = [0.62, 0.45, 0.36];
const FABRIC: [f32; 3] = [0.17, 0.36, 0.70];
const SELECTED_FABRIC: [f32; 3] = [0.95, 0.55, 0.25];

struct ClothOnGpu {
    mesh: GpuMesh,
    seq: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

/// What the 3D view shows besides the form.
pub enum Show<'a> {
    /// The drape, frame by frame.
    Drape(&'a SimFrame),
    /// The pieces being arranged, the selected one highlighted.
    Pieces {
        scene: &'a Rc<ArrangedScene>,
        selected: Option<PieceId>,
    },
}

/// The 3D image as drawn this frame: its response to the pointer, and the camera that drew it.
pub struct Drawn {
    pub response: egui::Response,
    pub camera: ScreenCamera,
}

/// The 3D panel: renders offscreen and shows the texture as an egui image.
pub struct Viewport {
    renderer: MeshRenderer,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    body: GpuMesh,
    cloth: Option<ClothOnGpu>,
    /// The arranged pieces on the GPU, and the scene they were made from.
    pieces: Vec<(PieceId, GpuMesh)>,
    pieces_of: Option<Rc<ArrangedScene>>,
    pub frames_drawn: u64,
}

impl Viewport {
    pub fn new(rs: &egui_wgpu::RenderState, stage: &Stage) -> Self {
        let renderer = MeshRenderer::new(&rs.device);
        let (positions, triangles) = stage.render_mesh();
        let body = renderer.create_mesh(&rs.device, &rs.queue, positions, triangles, SKIN);
        let camera = OrbitCamera {
            target: glam::Vec3::new(0.0, 0.95, 0.0),
            yaw: 0.5,
            pitch: 0.12,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        Self {
            renderer,
            camera,
            target: None,
            body,
            cloth: None,
            pieces: Vec::new(),
            pieces_of: None,
            frames_drawn: 0,
        }
    }

    pub fn camera(&self) -> &OrbitCamera {
        &self.camera
    }

    pub fn camera_mut(&mut self) -> &mut OrbitCamera {
        &mut self.camera
    }

    /// Turns the camera round to look at the form from `angle` (radians from its front towards
    /// its left), a little from above.
    pub fn look_from(&mut self, angle: f64) {
        self.camera.yaw = angle as f32;
        self.camera.pitch = 0.1;
    }

    /// Draws the form and `show`, and returns the image's response with the camera that drew
    /// it; None when the panel is too small to draw in.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rs: &egui_wgpu::RenderState,
        show: Show,
    ) -> Option<Drawn> {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let (w, h) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim)?;
        match show {
            Show::Drape(f) => {
                self.pieces.clear();
                self.pieces_of = None;
                self.sync_cloth(rs, f);
            }
            Show::Pieces { scene, selected } => {
                self.cloth = None;
                self.sync_pieces(rs, scene, selected);
            }
        }
        self.ensure_target(rs, w, h);
        let (target, texture_id) = self.target.as_ref().expect("ensure_target sets it");
        let mut meshes = vec![&self.body];
        meshes.extend(self.cloth.as_ref().map(|c| &c.mesh));
        meshes.extend(self.pieces.iter().map(|(_, m)| m));
        self.renderer.render(
            &rs.device,
            &rs.queue,
            target,
            self.camera.view_proj(w as f32 / h as f32),
            &meshes,
        );
        let texture_id = *texture_id;
        self.frames_drawn += 1;
        let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, size));
        let response = ui.add(image.sense(egui::Sense::click_and_drag()));
        let rect = response.rect;
        let camera = ScreenCamera::new(
            &self.camera,
            DVec2::new(f64::from(rect.min.x), f64::from(rect.min.y)),
            DVec2::new(f64::from(rect.width()), f64::from(rect.height())),
        );
        Some(Drawn { response, camera })
    }

    /// Uploads the arranged pieces when the scene changed, and colours the selected one.
    fn sync_pieces(
        &mut self,
        rs: &egui_wgpu::RenderState,
        scene: &Rc<ArrangedScene>,
        selected: Option<PieceId>,
    ) {
        if !self
            .pieces_of
            .as_ref()
            .is_some_and(|s| Rc::ptr_eq(s, scene))
        {
            self.pieces = scene
                .panels
                .iter()
                .map(|p| {
                    let positions: Vec<glam::Vec3> =
                        p.positions.iter().map(|q| q.as_vec3()).collect();
                    let mesh = self.renderer.create_mesh(
                        &rs.device,
                        &rs.queue,
                        &positions,
                        &p.triangles,
                        FABRIC,
                    );
                    (p.shape, mesh)
                })
                .collect();
            self.pieces_of = Some(scene.clone());
        }
        for (shape, mesh) in &mut self.pieces {
            mesh.set_color(if Some(*shape) == selected {
                SELECTED_FABRIC
            } else {
                FABRIC
            });
        }
    }

    /// Uploads a new simulation frame (and new triangles after welding) once per frame number.
    fn sync_cloth(&mut self, rs: &egui_wgpu::RenderState, f: &SimFrame) {
        match &mut self.cloth {
            Some(c) if c.seq == f.seq => {}
            Some(c) => {
                let new_tris =
                    (!Arc::ptr_eq(&c.triangles, &f.triangles)).then_some(f.triangles.as_slice());
                self.renderer.update_mesh(
                    &rs.device,
                    &rs.queue,
                    &mut c.mesh,
                    &f.positions,
                    new_tris,
                );
                c.seq = f.seq;
                c.triangles = f.triangles.clone();
            }
            None => {
                let mesh = self.renderer.create_mesh(
                    &rs.device,
                    &rs.queue,
                    &f.positions,
                    &f.triangles,
                    FABRIC,
                );
                self.cloth = Some(ClothOnGpu {
                    mesh,
                    seq: f.seq,
                    triangles: f.triangles.clone(),
                });
            }
        }
    }

    /// (Re)create the render target when the panel's pixel size changes, keeping the same egui texture id.
    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, w: u32, h: u32) {
        if self
            .target
            .as_ref()
            .is_some_and(|(t, _)| t.width == w && t.height == h)
        {
            return;
        }
        let target = RenderTarget::new(&rs.device, w, h);
        let mut renderer = rs.renderer.write();
        let id = match self.target.take() {
            Some((_, id)) => {
                renderer.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    &target.color_view,
                    wgpu::FilterMode::Linear,
                    id,
                );
                id
            }
            None => renderer.register_native_texture(
                &rs.device,
                &target.color_view,
                wgpu::FilterMode::Linear,
            ),
        };
        self.target = Some((target, id));
    }
}
