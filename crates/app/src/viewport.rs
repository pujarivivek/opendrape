use crate::sim_runner::SimFrame;
use crate::stage::Stage;
use opendrape_render::{GpuMesh, MeshRenderer, OrbitCamera, RenderTarget, target_size};
use std::sync::Arc;

/// Mid-brown skin tone and a cotton blue.
const SKIN: [f32; 3] = [0.62, 0.45, 0.36];
const FABRIC: [f32; 3] = [0.17, 0.36, 0.70];

struct ClothOnGpu {
    mesh: GpuMesh,
    seq: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

/// The 3D panel: renders offscreen and shows the texture as an egui image.
pub struct Viewport {
    renderer: MeshRenderer,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    body: GpuMesh,
    cloth: Option<ClothOnGpu>,
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
            frames_drawn: 0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, rs: &egui_wgpu::RenderState, frame: Option<&SimFrame>) {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let Some((w, h)) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim) else {
            return; // minimised or collapsed: nothing to draw
        };
        match frame {
            Some(f) => self.sync_cloth(rs, f),
            None => self.cloth = None,
        }
        self.ensure_target(rs, w, h);
        let (target, texture_id) = self.target.as_ref().expect("ensure_target sets it");
        let mut meshes = vec![&self.body];
        if let Some(c) = &self.cloth {
            meshes.push(&c.mesh);
        }
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
        let response = ui.add(image.sense(egui::Sense::drag()));
        let drag = response.drag_delta();
        if drag != egui::Vec2::ZERO {
            self.camera.drag(drag.x, drag.y);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll);
            }
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
