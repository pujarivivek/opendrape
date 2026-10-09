use crate::BodyMesh;

#[derive(Debug, PartialEq, Eq)]
pub enum OdbError {
    BadMagic,
    Truncated,
    IndexOutOfRange,
}

const MAGIC: &[u8; 8] = b"ODBODY01";

/// `ODBODY01`, vertex count (u32), triangle count (u32), then little-endian f32 xyz per
/// vertex and u32 × 3 per triangle.
pub fn write_odb(mesh: &BodyMesh) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 12 * (mesh.positions.len() + mesh.triangles.len()));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(mesh.positions.len() as u32).to_le_bytes());
    out.extend_from_slice(&(mesh.triangles.len() as u32).to_le_bytes());
    for p in &mesh.positions {
        for c in p.to_array() {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    for t in &mesh.triangles {
        for i in t {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    out
}

pub fn read_odb(bytes: &[u8]) -> Result<BodyMesh, OdbError> {
    if bytes.len() < 16 || &bytes[..8] != MAGIC {
        return Err(OdbError::BadMagic);
    }
    let word = |o: usize| -> [u8; 4] { bytes[o..o + 4].try_into().expect("4 bytes") };
    let (nv, nt) = (
        u32::from_le_bytes(word(8)) as usize,
        u32::from_le_bytes(word(12)) as usize,
    );
    if bytes.len() != 16 + 12 * (nv + nt) {
        return Err(OdbError::Truncated);
    }
    let f = |o: usize| f32::from_le_bytes(word(o));
    let positions = (0..nv)
        .map(|i| glam::Vec3::new(f(16 + 12 * i), f(20 + 12 * i), f(24 + 12 * i)))
        .collect();
    let base = 16 + 12 * nv;
    let mut triangles = Vec::with_capacity(nt);
    for i in 0..nt {
        let t = [0, 4, 8].map(|k| u32::from_le_bytes(word(base + 12 * i + k)));
        if t.iter().any(|&k| k as usize >= nv) {
            return Err(OdbError::IndexOutOfRange);
        }
        triangles.push(t);
    }
    Ok(BodyMesh {
        positions,
        triangles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn tiny() -> BodyMesh {
        BodyMesh {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn round_trips() {
        assert_eq!(read_odb(&write_odb(&tiny())), Ok(tiny()));
    }

    #[test]
    fn rejects_bad_magic_truncation_and_bad_indices() {
        assert_eq!(read_odb(b"not a body file at all"), Err(OdbError::BadMagic));
        let bytes = write_odb(&tiny());
        assert_eq!(
            read_odb(&bytes[..bytes.len() - 1]),
            Err(OdbError::Truncated)
        );
        let bad = BodyMesh {
            triangles: vec![[0, 1, 3]],
            ..tiny()
        };
        assert_eq!(read_odb(&write_odb(&bad)), Err(OdbError::IndexOutOfRange));
    }
}
