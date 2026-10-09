//! Builds OpenDrape bodies from CC0 MakeHuman data files (hm08 base mesh + targets).
//! Pure data parsing; no MakeHuman code is used.

use opendrape_body::BodyMesh;

/// The average young adult female: MakeHuman's three ethnicity targets blended equally
/// (their sliders sum to 1); height/proportion/breast targets are at weight 0 by default.
pub const FEMALE_AVERAGE: [(&str, f64); 3] = [
    ("african-female-young.target", 1.0 / 3.0),
    ("asian-female-young.target", 1.0 / 3.0),
    ("caucasian-female-young.target", 1.0 / 3.0),
];

/// A parsed OBJ: vertices (MakeHuman units: decimetres) and faces with their `g` group.
pub struct ObjMesh {
    pub verts: Vec<[f64; 3]>,
    pub faces: Vec<Vec<u32>>,
    pub groups: Vec<String>,
}

pub fn parse_obj(text: &str) -> ObjMesh {
    let (mut verts, mut faces, mut groups) = (vec![], vec![], vec![]);
    let mut group = String::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let c: Vec<f64> = parts
                    .take(3)
                    .map(|s| s.parse().expect("vertex coordinate"))
                    .collect();
                verts.push([c[0], c[1], c[2]]);
            }
            Some("f") => {
                faces.push(
                    parts
                        .map(|s| {
                            s.split('/')
                                .next()
                                .unwrap()
                                .parse::<u32>()
                                .expect("face index")
                                - 1
                        })
                        .collect(),
                );
                groups.push(group.clone());
            }
            Some("g") => group = parts.collect::<Vec<_>>().join(" "),
            _ => {}
        }
    }
    ObjMesh {
        verts,
        faces,
        groups,
    }
}

/// `v[i] += weight * (dx, dy, dz)` for each line `i dx dy dz`; returns lines applied.
pub fn apply_target(verts: &mut [[f64; 3]], target: &str, weight: f64) -> usize {
    let mut applied = 0;
    for line in target
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let p: Vec<&str> = line.split_whitespace().collect();
        let i: usize = p[0].parse().expect("target vertex index");
        for k in 0..3 {
            verts[i][k] += weight * p[k + 1].parse::<f64>().expect("target offset");
        }
        applied += 1;
    }
    applied
}

/// Faces of `group` only, reindexed in original vertex order; quads split along the shorter
/// diagonal (winding kept); decimetres → metres; lowest point moved to y = 0.
pub fn body_mesh(obj: &ObjMesh, group: &str) -> BodyMesh {
    let faces: Vec<&Vec<u32>> = obj
        .faces
        .iter()
        .zip(&obj.groups)
        .filter(|(_, g)| *g == group)
        .map(|(f, _)| f)
        .collect();
    let mut used: Vec<u32> = faces.iter().flat_map(|f| f.iter().copied()).collect();
    used.sort_unstable();
    used.dedup();
    let mut remap = vec![u32::MAX; obj.verts.len()];
    for (new, &old) in used.iter().enumerate() {
        remap[old as usize] = new as u32;
    }
    let d2 = |a: u32, b: u32| -> f64 {
        let (p, q) = (obj.verts[a as usize], obj.verts[b as usize]);
        (0..3).map(|k| (p[k] - q[k]).powi(2)).sum()
    };
    let mut triangles = vec![];
    for f in faces {
        let r = |k: u32| remap[k as usize];
        match f.as_slice() {
            &[a, b, c] => triangles.push([r(a), r(b), r(c)]),
            &[a, b, c, d] if d2(a, c) <= d2(b, d) => {
                triangles.extend([[r(a), r(b), r(c)], [r(a), r(c), r(d)]])
            }
            &[a, b, c, d] => triangles.extend([[r(a), r(b), r(d)], [r(b), r(c), r(d)]]),
            poly => triangles
                .extend((1..poly.len() - 1).map(|k| [r(poly[0]), r(poly[k]), r(poly[k + 1])])),
        }
    }
    // Scale in f64, then convert: identical bits on every platform (CI byte-compares the asset).
    let mut positions: Vec<glam::Vec3> = used
        .iter()
        .map(|&i| (glam::DVec3::from_array(obj.verts[i as usize]) * 0.1).as_vec3())
        .collect();
    let floor = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    for p in &mut positions {
        p.y -= floor;
    }
    BodyMesh {
        positions,
        triangles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBJ: &str = "# test\nv 0 0 0\nv 10 0 0\nv 10 20 0\nv 0 20 0\nv 5 5 5\nvt 0 0\ng body\nf 1/1 2/1 3/1 4/1\ng helper-tights\nf 1 2 5\n";

    #[test]
    fn parses_vertices_faces_and_groups() {
        let m = parse_obj(OBJ);
        assert_eq!(m.verts.len(), 5);
        assert_eq!(m.faces, vec![vec![0, 1, 2, 3], vec![0, 1, 4]]);
        assert_eq!(
            m.groups,
            vec!["body".to_string(), "helper-tights".to_string()]
        );
    }

    #[test]
    fn applies_weighted_target_offsets() {
        let mut v = vec![[0.0; 3]; 3];
        let n = apply_target(&mut v, "# comment\n\n1 1.0 2.0 -4.0\n", 0.5);
        assert_eq!(n, 1);
        assert_eq!(v[1], [0.5, 1.0, -2.0]);
        assert_eq!(v[0], [0.0; 3]);
    }

    #[test]
    fn keeps_body_group_triangulates_scales_and_grounds() {
        let b = body_mesh(&parse_obj(OBJ), "body");
        assert_eq!(b.positions.len(), 4, "helper-only vertex 5 dropped");
        // 10×20 dm quad: diagonals equal, so split along a-c: (a,b,c),(a,c,d)
        assert_eq!(b.triangles, vec![[0, 1, 2], [0, 2, 3]]);
        assert!(
            (b.positions[2] - glam::Vec3::new(1.0, 2.0, 0.0)).length() < 1e-6,
            "dm → m"
        );
        assert_eq!(
            b.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min),
            0.0
        );
    }

    #[test]
    fn splits_quads_along_the_shorter_diagonal() {
        // a=(0,0) b=(10,0) c=(30,10) d=(0,10): a–c is long, b–d is short → (a,b,d),(b,c,d)
        let obj = "v 0 0 0\nv 10 0 0\nv 30 10 0\nv 0 10 0\ng body\nf 1 2 3 4\n";
        assert_eq!(
            body_mesh(&parse_obj(obj), "body").triangles,
            vec![[0, 1, 3], [1, 2, 3]]
        );
    }
}
