//! Developer tasks: `cargo xtask icons` regenerates the app icons in `assets/`.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        _ => {
            eprintln!("usage: cargo xtask icons");
            std::process::exit(2);
        }
    }
}

/// A white A-line dress on a rounded terracotta square, 4×4 supersampled.
fn icons() {
    const S: u32 = 1024;
    let dress = [
        (430.0, 200.0),
        (470.0, 250.0),
        (554.0, 250.0),
        (594.0, 200.0),
        (640.0, 230.0),
        (610.0, 430.0),
        (780.0, 840.0),
        (244.0, 840.0),
        (414.0, 430.0),
        (384.0, 230.0),
    ];
    let img = image::RgbaImage::from_fn(S, S, |px, py| {
        let (mut bg, mut fg) = (0u32, 0u32);
        for sy in 0..4 {
            for sx in 0..4 {
                let x = px as f32 + (sx as f32 + 0.5) / 4.0;
                let y = py as f32 + (sy as f32 + 0.5) / 4.0;
                let (radius, margin) = (180.0f32, 64.0f32);
                let half = S as f32 / 2.0;
                let dx = (x - half).abs() - (half - margin - radius);
                let dy = (y - half).abs() - (half - margin - radius);
                if (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() <= radius {
                    if inside(&dress, x, y) {
                        fg += 1
                    } else {
                        bg += 1
                    }
                }
            }
        }
        let t = fg as f32 / (bg + fg).max(1) as f32;
        let mix = |a: f32, b: f32| (a * (1.0 - t) + b * t) as u8;
        image::Rgba([
            mix(196.0, 255.0),
            mix(92.0, 255.0),
            mix(56.0, 255.0),
            ((bg + fg) * 255 / 16) as u8,
        ])
    });
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    std::fs::create_dir_all(&root).expect("create assets/");
    img.save(root.join("icon@2x.png"))
        .expect("write icon@2x.png");
    image::DynamicImage::ImageRgba8(img)
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .save(root.join("icon.ico"))
        .expect("write icon.ico");
    println!("wrote assets/icon@2x.png and assets/icon.ico");
}

/// Even-odd point-in-polygon test.
fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[(i + poly.len() - 1) % poly.len()];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
    }
    c
}
