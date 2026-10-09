//! Developer tasks: `cargo xtask icons` regenerates the app icons in `assets/`;
//! `cargo xtask body` builds the bundled body from CC0 MakeHuman data.

mod makehuman;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        Some("body") => body(),
        _ => {
            eprintln!("usage: cargo xtask <icons|body>");
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

/// `cargo xtask body <makehuman_dir> <out.odb>`: average female body from CC0 MakeHuman data.
fn body() {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let [dir, out] = args.as_slice() else {
        eprintln!(
            "usage: cargo xtask body <makehuman_dir> <out.odb>   (run scripts/fetch-makehuman.sh first)"
        );
        std::process::exit(2);
    };
    let dir = std::path::Path::new(dir);
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let mut obj = makehuman::parse_obj(&read("base.obj"));
    for (name, weight) in makehuman::FEMALE_AVERAGE {
        let n = makehuman::apply_target(&mut obj.verts, &read(name), weight);
        println!("applied {name} × {weight:.4} ({n} vertices)");
    }
    let body = makehuman::body_mesh(&obj, "body");
    std::fs::write(out, opendrape_body::write_odb(&body)).expect("write .odb");
    println!(
        "wrote {out}: {} vertices, {} triangles, height {:.3} m, waist {:.3} m, hips {:.3} m, open edges {}",
        body.positions.len(),
        body.triangles.len(),
        opendrape_body::height(&body),
        opendrape_body::girth_at(&body, 1.028, 0.2),
        opendrape_body::girth_at(&body, 0.767, 0.2),
        opendrape_body::boundary_edge_count(&body),
    );
}
