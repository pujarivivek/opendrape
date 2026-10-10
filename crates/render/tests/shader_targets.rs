//! Every studio shader must translate for every graphics backend OpenDrape can fall back to.
//! OpenGL (GLSL ES 3.00, the WebGL2-class level the renderer keeps to) is the strictest: what
//! naga can't express there fails the pipeline at run time on the Linux and Windows GL
//! fallbacks, which this Mac-or-Windows CI otherwise never draws with.

const MODULES: [(&str, &str); 4] = [
    (
        "shade",
        concat!(
            include_str!("../src/studio/common.wgsl"),
            include_str!("../src/studio/shade.wgsl")
        ),
    ),
    (
        "ao",
        concat!(
            include_str!("../src/studio/common.wgsl"),
            include_str!("../src/studio/ao.wgsl")
        ),
    ),
    (
        "shadow",
        concat!(
            include_str!("../src/studio/common.wgsl"),
            include_str!("../src/studio/shadow.wgsl")
        ),
    ),
    (
        "output",
        concat!(
            include_str!("../src/studio/common.wgsl"),
            include_str!("../src/studio/output.wgsl")
        ),
    ),
];

#[test]
fn every_studio_shader_translates_to_glsl_es_300() {
    use naga::back::glsl;
    let mut failures = Vec::new();
    for (name, source) in MODULES {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        for entry in &module.entry_points {
            let options = glsl::Options {
                version: glsl::Version::new_gles(300),
                ..Default::default()
            };
            let pipeline = glsl::PipelineOptions {
                shader_stage: entry.stage,
                entry_point: entry.name.clone(),
                multiview: None,
            };
            let mut out = String::new();
            let written = glsl::Writer::new(
                &mut out,
                &module,
                &info,
                &options,
                &pipeline,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .and_then(|mut w| w.write());
            if let Err(e) = written {
                failures.push(format!("{name}::{}: {e}", entry.name));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "won't run on OpenGL:\n{}",
        failures.join("\n")
    );
}
