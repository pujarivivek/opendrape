//! The fabric every cloth is dressed in: unbleached muslin. Its weave is Poly Haven's
//! "Stretch Poplin" scan (CC0; see ASSETS.md), baked by `cargo xtask fabric` into one image:
//! red its brightness about the mean (at half scale), green and blue its normal map across and
//! along the weave, alpha its ambient occlusion. The studio lays it over cloth by each vertex's
//! place on the weave, in metres, at the scan's true size, so its threads are as fine as
//! poplin's.

use std::sync::LazyLock;

/// The scan's size (m): across the weft, then along the warp.
pub const SCAN_M: [f32; 2] = [0.291_02, 0.280_80];
/// The weave is laid at this many times the scan's size: muslin's threads are coarser than
/// poplin's, and at the scan's own size they vanish between the pixels of a whole garment.
pub const MUSLIN_SCALE: f32 = 2.0;
/// The weave's tile on the cloth (m).
pub const MUSLIN_TILE_M: [f32; 2] = [SCAN_M[0] * MUSLIN_SCALE, SCAN_M[1] * MUSLIN_SCALE];
/// How strongly the weave's relief tilts the light (1: as scanned).
pub const MUSLIN_RELIEF: f32 = 2.0;

static MUSLIN: LazyLock<image::RgbaImage> = LazyLock::new(|| {
    image::load_from_memory(include_bytes!("../../../../assets/fabric/muslin.png"))
        .expect("assets/fabric/muslin.png decodes")
        .to_rgba8()
});

/// The baked muslin image, decoded once.
pub fn muslin() -> &'static image::RgbaImage {
    &MUSLIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_muslin_is_baked_about_its_mean_with_a_flat_normal_on_average() {
        let m = muslin();
        assert!(
            m.width() >= 1024 && m.height() >= 900,
            "{}×{}",
            m.width(),
            m.height()
        );
        let n = (m.width() * m.height()) as f64;
        let mean = |k: usize| m.pixels().map(|p| f64::from(p[k])).sum::<f64>() / n / 255.0;
        assert!(
            (mean(0) - 0.5).abs() < 0.02,
            "brightness about its mean: {}",
            mean(0)
        );
        assert!(
            (mean(1) - 0.5).abs() < 0.03 && (mean(2) - 0.5).abs() < 0.03,
            "flat on average"
        );
        assert!(mean(3) > 0.5, "mostly lit: {}", mean(3));
    }
}
