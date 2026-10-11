use crate::arrange::{ArrangedScene, ScreenCamera};
use crate::sim_runner::SimFrame;
use crate::theme::{
    FABRIC, FORM_SRGB, METAL_SRGB, SEAM_SRGB, SELECTED_FABRIC, STAND_SRGB, TAPE_SRGB,
};
use crate::view_settings::{LightingChoice, QualityChoice, ViewSettings};
use glam::DVec2;
use opendrape_core::PieceId;
use opendrape_drape::{Fabric, FormLabel, Stage};
use opendrape_render::colour::srgb8_to_linear;
use opendrape_render::studio::quality::{self, Quality};
use opendrape_render::studio::{LabelImage, Material, StudioMesh, StudioRenderer};
use opendrape_render::{OrbitCamera, RenderTarget, target_size};
use std::rc::Rc;
use std::sync::Arc;

struct ClothOnGpu {
    mesh: StudioMesh,
    seq: u64,
    triangles: Arc<Vec<[u32; 3]>>,
    /// The fabric whose seam distances the mesh has (a new one each time an edit makes the
    /// fabric again).
    fabric: Arc<Fabric>,
}

/// A picture for Assets is drawn at most this many times while its still image builds up.
const THUMB_FRAMES: usize = 64;

/// A part of the dress form, drawn in its own colour and finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Torso,
    Seams,
    Tapes,
    Cap,
    Post,
    Label,
}

/// Edges of the stand sharper than this (radians) stay sharp.
const STAND_CREASE: f32 = 0.7;

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
    renderer: StudioRenderer,
    /// The quality level the graphics chip suggests (what Auto means here).
    auto_quality: Quality,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    /// The form's torso, tape lines and stand.
    form: Vec<(Part, StudioMesh)>,
    /// The pictures drawn for Assets, kept while their egui textures are shown.
    thumbs: Vec<RenderTarget>,
    /// The tape lines are drawn.
    show_tapes: bool,
    cloth: Option<ClothOnGpu>,
    /// The arranged pieces on the GPU, and the scene they were made from.
    pieces: Vec<(PieceId, StudioMesh)>,
    pieces_of: Option<Rc<ArrangedScene>>,
    pub frames_drawn: u64,
    /// A mouse button was held down on the image last frame.
    pointer_held: bool,
}

impl Viewport {
    pub fn new(
        rs: &egui_wgpu::RenderState,
        stage: &Stage,
        settings: ViewSettings,
        label: &[String; 3],
    ) -> Self {
        let mut renderer = StudioRenderer::new(&rs.device, &rs.adapter, Quality::Medium);
        let auto_quality = quality::auto(&rs.adapter.get_info(), renderer.hdr_ok());
        renderer.set_quality(settings.quality.resolve(auto_quality));
        renderer.set_lighting(settings.lighting.lighting());
        let form = form_meshes(&mut renderer, rs, stage, label);
        let camera = OrbitCamera {
            // The form's waist, in the middle of the view.
            target: glam::Vec3::new(0.0, stage.waist_y() as f32, 0.0),
            yaw: 0.5,
            pitch: 0.12,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        Self {
            renderer,
            auto_quality,
            camera,
            target: None,
            form,
            thumbs: Vec::new(),
            show_tapes: settings.show_tapes,
            cloth: None,
            pieces: Vec::new(),
            pieces_of: None,
            frames_drawn: 0,
            pointer_held: false,
        }
    }

    /// Draws the form of `stage` from now on, its size label reading `label`, and looks at its
    /// waist.
    pub fn set_stage(&mut self, rs: &egui_wgpu::RenderState, stage: &Stage, label: &[String; 3]) {
        self.form = form_meshes(&mut self.renderer, rs, stage, label);
        self.camera.target.y = stage.waist_y() as f32;
    }

    /// The form's size label reads `label` from now on.
    pub fn set_label(&mut self, rs: &egui_wgpu::RenderState, stage: &Stage, label: &[String; 3]) {
        if let (Some((_, mesh)), Some(frame)) = (
            self.form.iter_mut().find(|(part, _)| *part == Part::Label),
            stage.label(),
        ) {
            put_label(&mut self.renderer, rs, mesh, frame, label);
        }
    }

    /// Shows or hides the form's measuring tapes.
    pub fn set_show_tapes(&mut self, on: bool) {
        self.show_tapes = on;
    }

    /// A picture of `stage`'s form, `size` points large, for Assets: seen from the front
    /// three-quarter in the studio, its still image finished. The view's own next frame is drawn
    /// afresh.
    pub fn thumbnail(
        &mut self,
        rs: &egui_wgpu::RenderState,
        stage: &Stage,
        label: &[String; 3],
        size: egui::Vec2,
        pixels_per_point: f32,
    ) -> egui::TextureId {
        let (w, h) = (
            (size.x * pixels_per_point).round().max(1.0) as u32,
            (size.y * pixels_per_point).round().max(1.0) as u32,
        );
        let target = RenderTarget::new(&rs.device, w, h);
        let meshes = form_meshes(&mut self.renderer, rs, stage, label);
        let shown: Vec<&StudioMesh> = meshes
            .iter()
            .filter(|(part, _)| self.show_tapes || *part != Part::Tapes)
            .map(|(_, mesh)| mesh)
            .collect();
        let camera = OrbitCamera {
            target: glam::Vec3::new(0.0, stage.waist_y() as f32 + 0.05, 0.0),
            yaw: 0.5,
            pitch: 0.12,
            distance: 2.0,
            fov_y: 35f32.to_radians(),
        };
        self.renderer.set_moving(false);
        for _ in 0..THUMB_FRAMES {
            let drawn = self
                .renderer
                .render(&rs.device, &rs.queue, &target, &camera, &shown);
            if drawn.still_done {
                break;
            }
        }
        let id = rs.renderer.write().register_native_texture(
            &rs.device,
            &target.color_view,
            wgpu::FilterMode::Linear,
        );
        self.thumbs.push(target);
        id
    }

    /// The quality level the 3D view draws at.
    pub fn quality(&self) -> Quality {
        self.renderer.quality()
    }

    /// The level Auto picks on this computer.
    pub fn auto_quality(&self) -> Quality {
        self.auto_quality
    }

    pub fn set_quality_choice(&mut self, choice: QualityChoice) {
        self.renderer.set_quality(choice.resolve(self.auto_quality));
    }

    pub fn lighting(&self) -> opendrape_render::studio::Lighting {
        self.renderer.lighting()
    }

    pub fn set_lighting_choice(&mut self, choice: LightingChoice) {
        self.renderer.set_lighting(choice.lighting());
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
    /// `moving`: something the image can't show yet is moving (the drape plays, a drag is
    /// held), so the view shouldn't start building its finished still image.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rs: &egui_wgpu::RenderState,
        show: Show,
        moving: bool,
    ) -> Option<Drawn> {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let (w, h) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim)?;
        // Drawn at no more pixels than the quality level allows, and scaled up to fill.
        let (w, h) = self.renderer.render_size(w, h);
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
        let mut meshes: Vec<&StudioMesh> = self
            .form
            .iter()
            .filter(|(part, _)| self.show_tapes || *part != Part::Tapes)
            .map(|(_, mesh)| mesh)
            .collect();
        meshes.extend(self.cloth.as_ref().map(|c| &c.mesh));
        meshes.extend(self.pieces.iter().map(|(_, m)| m));
        // A button held on the view (as it was last frame) is a drag in progress too.
        self.renderer.set_moving(moving || self.pointer_held);
        let rendered = self
            .renderer
            .render(&rs.device, &rs.queue, target, &self.camera, &meshes);
        let texture_id = *texture_id;
        if rendered.drew {
            self.frames_drawn += 1;
        }
        // Still frames build up the finished image one per frame; then it stops asking.
        if !rendered.still_done {
            ui.ctx().request_repaint();
        }
        let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, size));
        let response = ui.add(image.sense(egui::Sense::click_and_drag()));
        self.pointer_held = response.is_pointer_button_down_on();
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
                        Material::Cloth,
                    );
                    (p.shape, mesh)
                })
                .collect();
            self.pieces_of = Some(scene.clone());
        }
        for (shape, mesh) in &mut self.pieces {
            mesh.set_colour(if Some(*shape) == selected {
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
                let new_fabric = !Arc::ptr_eq(&c.fabric, &f.fabric);
                if new_fabric {
                    c.mesh.set_seams(&f.fabric.seam_mm());
                    c.mesh
                        .set_weave(&f.fabric.weave_m(), &f.fabric.cut_triangles());
                    c.fabric = f.fabric.clone();
                }
                if new_fabric || new_tris.is_some() {
                    // A weld joins particles: each vertex follows the one its own has become.
                    c.mesh.set_corners(&f.fabric.live_map(&f.triangles));
                }
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
                let mut mesh = self.renderer.create_mesh(
                    &rs.device,
                    &rs.queue,
                    &f.positions,
                    &f.triangles,
                    FABRIC,
                    Material::Cloth,
                );
                mesh.set_seams(&f.fabric.seam_mm());
                mesh.set_weave(&f.fabric.weave_m(), &f.fabric.cut_triangles());
                mesh.set_corners(&f.fabric.live_map(&f.triangles));
                self.renderer.update_mesh(
                    &rs.device,
                    &rs.queue,
                    &mut mesh,
                    &f.positions,
                    Some(&f.triangles),
                );
                self.cloth = Some(ClothOnGpu {
                    mesh,
                    seq: f.seq,
                    triangles: f.triangles.clone(),
                    fabric: f.fabric.clone(),
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

/// The parts of `stage`'s form on the GPU, each in its colour and finish (a part with no
/// triangles is left out): the linen torso and seams, the measuring tapes, the metal cap, the
/// dark post, and the woven size label reading `label`.
fn form_meshes(
    renderer: &mut StudioRenderer,
    rs: &egui_wgpu::RenderState,
    stage: &Stage,
    label: &[String; 3],
) -> Vec<(Part, StudioMesh)> {
    let mut meshes: Vec<(Part, StudioMesh)> = [
        (Part::Torso, stage.render_mesh(), FORM_SRGB, Material::Linen),
        (Part::Seams, stage.seams_mesh(), SEAM_SRGB, Material::Linen),
        (Part::Tapes, stage.tapes_mesh(), TAPE_SRGB, Material::Form),
        (Part::Cap, stage.cap_mesh(), METAL_SRGB, Material::Metal),
        (Part::Post, stage.stand_mesh(), STAND_SRGB, Material::Form),
    ]
    .into_iter()
    .filter(|(_, (_, triangles), _, _)| !triangles.is_empty())
    .map(|(part, (positions, triangles), colour, material)| {
        // The stand's pieces are solids with sharp rims: keep them sharp.
        let (positions, triangles) = if matches!(part, Part::Cap | Part::Post) {
            opendrape_render::split_creases(positions, triangles, STAND_CREASE)
        } else {
            (positions.to_vec(), triangles.to_vec())
        };
        let mesh = renderer.create_mesh(
            &rs.device,
            &rs.queue,
            &positions,
            &triangles,
            srgb8_to_linear(colour),
            material,
        );
        (part, mesh)
    })
    .collect();
    if let Some(frame) = stage.label() {
        let mut mesh = renderer.create_mesh(
            &rs.device,
            &rs.queue,
            &frame.positions,
            &frame.triangles,
            [1.0; 3],
            Material::Label,
        );
        put_label(renderer, rs, &mut mesh, frame, label);
        meshes.push((Part::Label, mesh));
    }
    meshes
}

/// Draws the size label reading `lines` onto the label's mesh.
fn put_label(
    renderer: &mut StudioRenderer,
    rs: &egui_wgpu::RenderState,
    mesh: &mut StudioMesh,
    frame: &FormLabel,
    lines: &[String; 3],
) {
    let picture = crate::form_label::picture(lines);
    renderer.set_label(
        &rs.device,
        &rs.queue,
        mesh,
        &LabelImage {
            image: &picture,
            centre: frame.centre,
            right: frame.right,
            up: frame.up,
            size: frame.size,
        },
    );
}
