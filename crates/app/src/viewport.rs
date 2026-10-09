use opendrape_render::{CubeRenderer, OrbitCamera, RenderTarget, target_size};

/// The 3D panel: renders offscreen and shows the texture as an egui image.
pub struct Viewport {
    cube: CubeRenderer,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    pub frames_drawn: u64,
}

impl Viewport {
    pub fn new(rs: &egui_wgpu::RenderState) -> Self {
        Self {
            cube: CubeRenderer::new(&rs.device),
            camera: OrbitCamera::default(),
            target: None,
            frames_drawn: 0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, rs: &egui_wgpu::RenderState) {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let Some((w, h)) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim) else {
            return; // minimised or collapsed: nothing to draw
        };
        self.ensure_target(rs, w, h);
        let (target, texture_id) = self.target.as_ref().expect("ensure_target sets it");
        self.cube.render(
            &rs.device,
            &rs.queue,
            target,
            self.camera.view_proj(w as f32 / h as f32),
        );
        self.frames_drawn += 1;

        let image = egui::Image::new(egui::load::SizedTexture::new(*texture_id, size));
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
