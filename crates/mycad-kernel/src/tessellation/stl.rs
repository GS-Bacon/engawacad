use super::TriangleMesh;
use std::fmt::Write;

pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
    let tri_count = mesh.triangle_count();
    let mut out = String::with_capacity(64 + tri_count * 200);

    writeln!(out, "solid {}", solid_name).unwrap();

    for tri in 0..tri_count {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;

        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];

        let n = face_normal(p0, p1, p2);

        writeln!(out, "  facet normal {} {} {}", n[0], n[1], n[2]).unwrap();
        writeln!(out, "    outer loop").unwrap();
        writeln!(out, "      vertex {} {} {}", p0[0], p0[1], p0[2]).unwrap();
        writeln!(out, "      vertex {} {} {}", p1[0], p1[1], p1[2]).unwrap();
        writeln!(out, "      vertex {} {} {}", p2[0], p2[1], p2[2]).unwrap();
        writeln!(out, "    endloop").unwrap();
        writeln!(out, "  endfacet").unwrap();
    }

    writeln!(out, "endsolid {}", solid_name).unwrap();
    out
}

fn face_normal(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3]) -> [f64; 3] {
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let nx = u[1] * v[2] - u[2] * v[1];
    let ny = u[2] * v[0] - u[0] * v[2];
    let nz = u[0] * v[1] - u[1] * v[0];
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 0.0 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, 0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::primitives::make_cuboid;
    use crate::tessellation::tessellate_solid;

    #[test]
    fn ascii_stl_cuboid_structure() {
        let mut g = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
        let mesh = tessellate_solid(&solid).unwrap();
        let stl = to_ascii_stl(&mesh, "test_cube");

        assert!(stl.starts_with("solid test_cube\n"));
        assert!(stl.trim_end().ends_with("endsolid test_cube"));
        assert_eq!(stl.matches("facet normal").count(), 12);
        assert_eq!(stl.matches("endfacet").count(), 12);
        assert_eq!(stl.matches("outer loop").count(), 12);
        assert_eq!(stl.matches("endloop").count(), 12);
        assert_eq!(stl.matches("\n      vertex ").count(), 36);
    }

    #[test]
    fn ascii_stl_deterministic() {
        let run = || {
            let mut g = IdGenerator::new(0);
            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
            to_ascii_stl(&tessellate_solid(&s).unwrap(), "cube")
        };
        assert_eq!(run(), run());
    }
}
