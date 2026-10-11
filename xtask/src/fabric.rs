//! `cargo xtask fabric`: bakes the muslin's scan into `assets/fabric/muslin.png`, the one
//! image the 3D view dresses cloth with. Red is the scan's brightness (linear, relative to its
//! mean, at half scale: 0.5 is the mean, 1.0 twice it), green and blue the weave's normal
//! across and along the cloth (as the scan's OpenGL normal map has them), alpha its ambient
//! occlusion. The scan is Poly Haven "Stretch Poplin" at 1k, fetched by
//! `scripts/fetch-fabric.sh` into `target/fabric/`.

use image::{Rgba, RgbaImage};
use std::path::Path;

/// sRGB to linear.
fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn run(args: Vec<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let dir = root.join("target/fabric");
    let name = args.first().map_or("stretch_poplin", String::as_str);
    let open = |map: &str| {
        let path = dir.join(format!("{name}_{map}_1k.jpg"));
        image::open(&path).unwrap_or_else(|e| {
            eprintln!(
                "{}: {e} (run scripts/fetch-fabric.sh first)",
                path.display()
            );
            std::process::exit(1);
        })
    };
    let diff = open("diff").to_rgb8();
    let nor = open("nor_gl").to_rgb8();
    let ao = open("ao").to_luma8();
    let (w, h) = diff.dimensions();
    assert_eq!(nor.dimensions(), (w, h), "the maps are the same size");
    assert_eq!(ao.dimensions(), (w, h), "the maps are the same size");
    let lum: Vec<f64> = diff
        .pixels()
        .map(|p| 0.2126 * linear(p[0]) + 0.7152 * linear(p[1]) + 0.0722 * linear(p[2]))
        .collect();
    let mean = lum.iter().sum::<f64>() / lum.len() as f64;
    let (lo, hi) = lum
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let out = RgbaImage::from_fn(w, h, |x, y| {
        let rel = (lum[(y * w + x) as usize] / mean * 0.5).clamp(0.0, 1.0);
        let n = nor.get_pixel(x, y);
        Rgba([
            (rel * 255.0).round() as u8,
            n[0],
            n[1],
            ao.get_pixel(x, y)[0],
        ])
    });
    let dest = root.join("assets/fabric");
    std::fs::create_dir_all(&dest).expect("create assets/fabric/");
    out.save(dest.join("muslin.png")).expect("write muslin.png");
    println!(
        "wrote assets/fabric/muslin.png ({w}×{h}) from {name}: brightness mean {mean:.4}, \
         {:.2}..{:.2} of it",
        lo / mean,
        hi / mean
    );
}
