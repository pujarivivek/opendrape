# M5b Studio view: a better-looking 3D view (design)

## Why

The 3D view today is one simple pass:
- flat colours with fixed lighting;
- no floor, no shadows, no ambient occlusion;
- an approximate gamma curve that clips bright areas.

Cloth looks like plastic and the form floats in grey. Students need to see folds, judge how a garment
sits on the form, and **trust the colour they see**. The user asked for ambient occlusion,
image-based studio lighting and soft contact shadows, and for more research. All of it has to stay
smooth on 4–8 GB laptops with integrated graphics.

**Decisions confirmed with the user (2026-10-10):**
- *Smooth while moving, sharp when still.* A lighter version runs while the view turns or the cloth
  drapes. The full look builds up over about a second once everything stops. The app picks a
  starting quality from the graphics chip, and the user can change it.
- *Soft grey photo studio.* A light-grey floor that fades seamlessly into a soft grey backdrop, from
  every angle.
- Approach 1 from the research: a full studio renderer in quality levels.

## What the student sees

- **Studio:** a light-grey floor that fades into a soft grey backdrop, a little lighter at the
  horizon. No visible edge from any camera angle.
- **Lighting:** a neutral photo studio.
  - The diffuse light comes from the CC0 studio HDRI "Studio Small 08" (Poly Haven, Sergej Majboroda;
    softboxes and umbrellas, low contrast, 6000 K), corrected to pure neutral white.
  - One soft key light, set in the HDRI's own brightest direction, adds gentle highlights and casts
    the shadows.
- **Colour you can trust:** a matte fabric card facing the camera shows its own colour.
  - Khronos PBR Neutral tone mapping maps colours 1:1 up to bright highlights.
  - The output uses exact sRGB encoding.
  - Exposure is calibrated so the camera-facing light level reproduces albedo.
- **Ambient occlusion:** soft darkening in folds, at seams, under the arms, and where the garment meets
  the form and the floor.
  - It darkens only the ambient light.
  - A multi-bounce correction keeps pale fabrics from going grey.
  - Flat surfaces are never darkened.
- **Soft shadows:**
  - a soft contact shadow on the floor under the form and garment;
  - soft key-light shadows across the cloth and form. No hard edges.
- **Cloth that reads as fabric:**
  - diffuse shading wraps softly round the folds;
  - a sheen brightens the fabric at grazing angles (Charlie sheen, as in Filament and glTF
    `KHR_materials_sheen`);
  - no plastic highlight;
  - the inside of a garment shows a little darker, like a lining.
- **The form:** matte linen beige (sRGB 188, 168, 153), no sheen. This matches the dress-form look the
  user approved for Track B.
- **Clean edges:**
  - FXAA while moving.
  - Once still, about 16 jittered frames are averaged. This is supersampling: thin cloth edges and
    shadow soft spots come out clean.
  - After that the view stops redrawing until something changes, which saves battery.
- **View → 3D quality:** *Auto (Medium)*, *Basic*, *Medium*, *High*. The choice is remembered between
  launches.

## Quality levels

"Moving" means:
- the camera changed this frame;
- the drape is playing;
- a gizmo, pin or fabric drag is held;
- or the pattern changed.

Otherwise the view is "still", and the still extras accumulate.

| Level | Chosen automatically for | Moving | Added when still | Render size cap |
|---|---|---|---|---|
| **Basic** | software rendering (WARP, llvmpipe), unknown or virtual GPUs, and the GL fallback without float render targets | Studio lighting, sheen, tone mapping, floor contact shadow; no AO, no key shadows, no FXAA | AO, key shadows (1024), 16-frame supersampling | 1 Mpx |
| **Medium** | integrated GPUs | AO at half resolution (8 samples), key shadows 1024 (8 taps), FXAA | AO samples rotate per frame; 16-frame supersampling | 2 Mpx |
| **High** | discrete GPUs, and Apple GPUs | AO at half resolution (12 samples), key shadows 2048 (12 taps), FXAA | full-resolution AO; 32-frame supersampling | 4 Mpx |

- **Render size cap:** the 3D image is rendered at no more than the cap, in physical pixels. egui
  scales it up to the panel, so a large Retina panel doesn't cost a laptop 150 MB of render targets.
- **Shadow and floor maps:** they depend only on the geometry, so they are redrawn only when the
  geometry changes, never just because the camera moved.

## Design

### Pass order (one frame)

1. **Key shadow map.** An orthographic depth render from the key light (`Depth32Float`), for the form
   and the cloth with culling off. Redrawn only when the geometry changes.
2. **Floor contact map.** An orthographic depth render from below, converted to opacity by height and
   blurred twice (model-viewer's technique: 256² on Basic, 512² otherwise). Redrawn only when the
   geometry changes.
3. **Prepass.** Full-resolution depth (`Depth32Float`, sampled with `textureLoad`) plus view-space normals
   (`Rgba8Unorm`), for the form, cloth and floor.
4. **AO.** A GTAO-style horizon search in a fragment shader into `R8Unorm` at half resolution (full on
   High when still). 4×4 interleaved noise, then a depth-aware blur in two passes. The main pass
   upsamples it with depth weights.
5. **Main pass** into the HDR colour target. It uses depth test `Equal` against the prepass, and the
   backdrop is drawn where depth is 1. It computes:
   - **diffuse:** SH9 irradiance × multi-bounce(AO), plus key light × PCF shadow, wrapped for cloth;
   - **sheen:** Charlie D with Neubelt visibility, lit by the key light and SH. The base layer is
     scaled by the sheen albedo approximation.
   - **floor:** albedo × (ambient × contact shadow × AO + key light × shadow), fading into the backdrop
     colour with distance;
   - **back faces of cloth:** the normal flipped and the albedo × 0.8 (the lining look).
6. **Output.**
   - **Moving:** PBR Neutral × exposure, then sRGB encode, then FXAA, into the `Rgba8Unorm` target egui
     shows.
   - **Still:** the jittered HDR frame is averaged into an accumulation target (ping-pong), then tone
     mapped and encoded from the average.

### Formats and fallbacks
- **HDR colour and accumulation:** `Rgba16Float` when the adapter reports it renderable
  (`adapter.get_texture_format_features`). Otherwise `Rgba8Unorm`, and the main pass tone maps itself
  (the "LDR path"). That path also accumulates in 8 bits.
- **No MSAA anywhere.** GLES 3.0 can't sample multisampled depth for AO. FXAA plus still-time
  supersampling covers anti-aliasing.
- **Limits:** everything stays within `Limits::downlevel_webgl2_defaults()`:
  - fragment passes only, no compute or storage;
  - at most 2 colour attachments per pass;
  - textures ≤ 2048.

### Studio environment (baked, not loaded at run time)
- **The HDRI:** fetched by `scripts/fetch-studio-hdri.sh` from
  `https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/studio_small_08_1k.hdr` (1,508,872 bytes,
  MD5 `de3ba64222895aca876b1d1c2e0cf81a`, checked) into `target/`. It is never committed.
- **`cargo xtask studio <hdr> <out.rs>`** projects the HDRI onto SH9 (27 floats), then:
  - re-white-balances it so the L0 band is neutral grey;
  - takes the key-light direction from the dominant L1 direction;
  - moves 40 % of the light into the key light and subtracts its SH projection, so the total stays
    the same.

  It writes `crates/render/src/studio/environment_data.rs` (committed, about 40 lines).
- **CI:** an `assets` job re-fetches the HDRI, re-bakes it and compares the result byte for byte, like
  the body today. `ASSETS.md` gets a row.

### Code structure
| Unit | Role |
|---|---|
| `crates/render/src/studio/mod.rs` | `StudioRenderer`, which takes the place of `MeshRenderer` in the app.<br>Same `create_mesh` / `update_mesh`, plus a `Material` per mesh (`Cloth { colour }`, `Form { colour }`).<br>`render(device, queue, target, camera, meshes, motion, quality) -> Rendered { still_done: bool }` |
| `studio/quality.rs` | `Quality { Basic, Medium, High }`; `Quality::auto(&AdapterInfo, hdr_ok)`; per-level `Settings` (sample counts, map sizes, render cap, frames to accumulate) |
| `studio/targets.rs` | All intermediate textures, recreated on resize or quality change |
| `studio/shadow.rs` + `shadow.wgsl` | Key shadow map and floor contact map, with a geometry version to skip redraws |
| `studio/ao.rs` + `ao.wgsl` | Prepass, AO and blur |
| `studio/shade.wgsl` | Main pass: backdrop, floor, form, cloth |
| `studio/output.rs` + `output.wgsl` | Accumulation, PBR Neutral, sRGB, FXAA |
| `studio/environment.rs` + `environment_data.rs` | SH evaluation, key light, calibrated exposure |
| `crates/render/src/colour.rs` | CPU mirrors used by tests and calibration: sRGB↔linear, PBR Neutral, ΔE2000 |
| `crates/app/src/viewport.rs` | Builds the frame's `Motion` from the camera, cloth seq, scene, selection, drags and size. Asks egui for repaints while accumulating, and skips rendering once still is done |
| `crates/app/src/view_settings.rs` | The 3D quality choice (Auto or a level) saved as JSON in the config directory beside the GPU state |
| `crates/app/src/app.rs` | View → 3D quality submenu (radio items; Auto shows the level it picked) |
| `xtask/src/studio.rs` | The bake (adds the `image` crate's `hdr` decoder to xtask only) |

`MeshRenderer`, `vertex_normals`, `RenderTarget`, `read_back` and `OrbitCamera` stay public. The
existing render tests keep testing `MeshRenderer`, apart from a background check that changes.

### Motion and accumulation
- **The frame signature** covers:
  - the camera (target, yaw, pitch, distance), the image size and the quality;
  - the cloth `seq`;
  - the arranged-scene `Rc` pointer and the selected piece;
  - whether a drag is held, and whether the drape is playing.
- **A frame counts as moving** when its signature differs from the last frame's.
- **While still,** each frame adds one jittered sample (a Halton(2,3) sub-pixel offset) with the AO and
  shadow noise rotated. Once the level's frame count is reached, `still_done` is true, and the app
  stops rendering and stops asking for repaints. The egui texture keeps the last image.
- **Overlays:** the jitter is sub-pixel and egui draws them from the unjittered camera, so the gizmo,
  pins and readouts don't move.

## Out of scope
- Reflection or specular cubemaps. Cloth barely reflects; the SH plus key light are enough.
- Weave and normal textures (Texturing), and photoreal or AI rendering (the Rendering workspace).
- MSAA, reprojected TAA, and screen-space reflections.

## Testing

**Headless render tests** (`crates/render/tests/studio.rs`). They run on llvmpipe (Linux CI), Metal
(macOS) and WARP (Windows):
- **Colour you can trust:** a still, accumulated render of a matte card facing the camera, in 6
  colours (18 % grey, cotton red, grass green, navy, skin beige, mustard), each displays within
  **ΔE2000 ≤ 3** of its sRGB colour. Repeated on the LDR path with ΔE2000 ≤ 5.
- **AO darkens folds, not flats:** in a V-shaped crease mesh, the crease bottom is at least 15 %
  darker with AO than without. An open floor patch is within 2 % either way.
- **Contact shadow:** a box above the floor. The floor directly under it is at least 20 % darker than
  open floor 1 m away, and the edge is soft (at least 6 px from 10 % to 90 %).
- **Still means still:**
  - with an unchanged signature, `still_done` turns true after the level's frame count;
  - a changed camera resets it;
  - a changed cloth `seq` redraws the shadow maps, and a camera-only change does not (counted).
- **Fallbacks:** forcing `hdr_ok = false` still renders and passes the LDR colour check.
- **Quality:** each level renders without validation errors at 1×1, 37×19 and 1000×700 targets.

**Unit tests:**
- PBR Neutral CPU mirror: identity below the threshold, and hue kept above it.
- sRGB round trip.
- ΔE2000 against the published Sharma test pairs.
- `Quality::auto` for each device type and backend.
- SH: a constant environment gives L0 only, and white balance makes L0 neutral (xtask).

**App tests:**
- View → 3D quality switches the level, and the choice is saved and read back.
- The 3D view stops rendering once still (frame counter) and restarts when you orbit or draping
  starts.
- Existing tests stay green. `crates/render/tests/mesh_render.rs`'s `is_background` follows the new
  backdrop only if it uses `StudioRenderer`; it keeps `MeshRenderer`, so it doesn't change.

**CI:**
- the studio bake check;
- the Windows WARP smoke test still exits within its 60 s timeout on Basic.

**User checklist:** `docs/testing/M5b-checklist.md`:
- the folds look soft and grounded;
- the floor shadow is under the form;
- colours match a reference swatch on screen;
- the view sharpens when you stop;
- View → 3D quality changes the look, and Basic stays smooth on a weak laptop.

## Risks
- **Unmeasured cost on Intel UHD 620.** The research estimates are about 5–7 ms for Basic plus moving
  extras, and 9–12 ms for Medium. Mitigations: the Auto level is conservative, the render-size caps,
  shadow maps are redrawn only on geometry change, and the user can switch levels.
- **GL drivers without float render targets** get the LDR path.
- **AO halos at the silhouette.** Mitigated by the depth-aware blur and a falloff by view-space
  distance; checked by the flat-floor test.
- **Thin two-sided cloth self-shadowing (acne):** normal-offset bias plus slope-scaled bias in the
  shadow pass.
