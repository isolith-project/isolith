//! SOLIDCORE-SEAM-EVAL capability probe (2026-10-27).
//!
//! Drives the solidcore 0.0.x kernel through exactly the operations oep-geo's
//! kernel seam requires, on the same geometry the Isolith OCCT path already
//! passes, and asserts the same acceptance bar: volume within 1e-6 RELATIVE of
//! the analytic / committed OCC golden value (`kernel-seam-golden.json`).
//!
//! Ops covered (mirrors crates/oep-geo/tests/kernel_seam.rs + the reference
//! bracket fixture `example-bracket.oepgeo.json`):
//!   1. box-reference unit cube                      (primitive)
//!   2. union / difference / intersection of two half-overlapping unit cubes
//!   3. extrude of the unit-square footprint          (sweep)
//!   4. revolve of a meridian (r in [0.4,1], h=1)    (sweep, full turn)
//!   5. fillet of one box edge, r=0.2                (blend)
//!   6. chamfer, distance 0.2                        (blend; scope note below)
//!   7. bracket proto: 80x40x6mm plate + 6mm hole (boolean subtract) + fillet
//!   8. typed-error probes (no panics on bad input)
//!
//! Scope notes reported, not hidden:
//!   - solidcore counts unique topology ids (a box = 6 faces / 12 edges / 8
//!     vertices); the OCCT path reports OCCT occurrence counts (6/24/48). Face
//!     count and volume compare directly; edge/vertex counts do not (convention
//!     difference, recorded in the evaluation memo).
//!   - the 0.0.x API has no per-edge chamfer entry point (`SmApiChamferFillet`
//!     chamfers every blendable edge). The chamfer probe runs that op and
//!     reports its measured result, which is NOT comparable to the OCCT
//!     single-edge chamfer golden.
//!
//! Build/run: `cargo run --release` (own workspace; pins solidcore rev
//! 3c931f4c6f85a026b186954d5742152b7de8b42a).

use solidcore::blend::fillet_executive::api as fillet_api;
use solidcore::boolean::api_brep as boolean;
use solidcore::curve::curve::Curve;
use solidcore::geom::attribute::{AttributeStore, ObjectKey};
use solidcore::kernel::Kernel;
use solidcore::mass::integrator::properties::{brep_volume, PropertyAttributes};
use solidcore::math::{Point3, Vec3};
use solidcore::primitive::api_curves as curves;
use solidcore::primitive::api_primitives as primitives;
use solidcore::query::api_queries::get_edges;
use solidcore::sweep::api_brep as sweeps;
use solidcore::topology::brep::p1 as bl;
use solidcore::topology::edge as el;
use solidcore::topology::face::p2 as fl;
use solidcore::topology::Brep;

const VOL_ACCURACY: f64 = 1e-8;
const REL_BAR: f64 = 1e-6;

/// Volume of an already-built solid plus unique topology counts.
fn measure(k: &Kernel, b: &mut Brep) -> Result<(f64, usize, usize, usize, usize), String> {
    let faces = bl::faces(b, None).map_err(|e| e.to_string())?;
    let edges = bl::edges(b, None).map_err(|e| e.to_string())?;
    let vertices = bl::vertices(b, None).map_err(|e| e.to_string())?;
    let mut loops = 0usize;
    for f in &faces {
        loops += fl::loops(b, *f, false, None).map_err(|e| e.to_string())?.len();
    }
    let store = AttributeStore::new();
    let key_of = |_| ObjectKey(0);
    let attributes = PropertyAttributes {
        store: &store,
        key_of: &key_of,
    };
    let volume = brep_volume(k, b, VOL_ACCURACY, &attributes).map_err(|e| e.to_string())?;
    Ok((volume, faces.len(), edges.len(), vertices.len(), loops))
}

fn measure_owned(k: &Kernel, b: Brep) -> Result<(f64, usize, usize, usize, usize), String> {
    let mut b = b;
    measure(k, &mut b)
}

fn pick_one_edge(b: &Brep) -> Result<solidcore::topology::EdgeId, String> {
    for id in get_edges(b).map_err(|e| e.to_string())? {
        let fs = el::faces(b, id, None).map_err(|e| e.to_string())?;
        if fs.len() >= 2 {
            return Ok(id);
        }
    }
    Err("no blendable edge found".to_string())
}

fn profile(points: &[(f64, f64, f64)]) -> Result<Vec<Curve>, String> {
    let mut out = Vec::new();
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        let l = curves::create_line_segment(
            Point3::new(a.0, a.1, a.2),
            Point3::new(b.0, b.1, b.2),
        )
        .map_err(|e| e.to_string())?;
        out.push(Curve::from(l));
    }
    Ok(out)
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

fn short(s: &str) -> String {
    s.chars().take(100).collect()
}

fn main() {
    let mut k = Kernel::default();
    let mut outcomes: Vec<(String, bool)> = Vec::new();

    println!("=== SOLIDCORE capability probe (solidcore 0.0.1 @ 3c931f4) ===");
    println!("tolerance bar: relative volume < {REL_BAR} vs analytic / OCC kernel-seam golden");
    println!("volume integration accuracy: {VOL_ACCURACY} (upstream clamp [1e-8, 1e-1])");
    println!("--- OCCT kernel-seam golden (oep-geo tests/golden) for reference ----");
    for (k0, g) in [
        ("box", 0.9999999999999998),
        ("union", 1.4999999999999993),
        ("difference", 0.4999999999999999),
        ("intersection", 0.4999999999999999),
        ("extrude", 0.9999999999999998),
        ("revolve", 2.638937829015426),
        ("fillet", 0.9914159265358977),
        ("chamfer", 0.98),
    ] {
        println!("  occ {k0:>12}: volume={g}");
    }
    println!("----------------------------------------------------------------");

    // 1. box-reference unit cube
    match primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0)
        .map_err(|e| e.to_string())
        .and_then(|b| measure_owned(&k, b))
    {
        Ok((vol, f, e, v, l)) => {
            let pass = rel(vol, 1.0) < REL_BAR;
            println!("{}  {:<26} volume={vol:.12} faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "box-reference");
            outcomes.push(("box-reference".to_string(), pass));
        }
        Err(err) => {
            println!("FAIL  {:<26} ERROR: {err}", "box-reference");
            outcomes.push(("box-reference".to_string(), false));
        }
    }

    // 2. booleans on half-overlapping unit cubes (analytic 1.5 / 0.5 / 0.5)
    {
        let a = primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let b = primitives::create_box(&mut k, Point3::new(0.5, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        match boolean::boolean_union(&mut k, a, b)
            .map_err(|e| e.to_string())
            .and_then(|brep| measure_owned(&k, brep))
        {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, 1.5) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} expected=1.5   faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "union");
                outcomes.push(("union".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "union");
                outcomes.push(("union".to_string(), false));
            }
        }
    }
    {
        let a = primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let b = primitives::create_box(&mut k, Point3::new(0.5, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        match boolean::boolean_difference(&mut k, a, b)
            .map_err(|e| e.to_string())
            .and_then(|brep| measure_owned(&k, brep))
        {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, 0.5) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} expected=0.5   faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "difference");
                outcomes.push(("difference".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "difference");
                outcomes.push(("difference".to_string(), false));
            }
        }
    }
    {
        let a = primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let b = primitives::create_box(&mut k, Point3::new(0.5, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        match boolean::boolean_intersection(&mut k, a, b)
            .map_err(|e| e.to_string())
            .and_then(|brep| measure_owned(&k, brep))
        {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, 0.5) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} expected=0.5   faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "intersection");
                outcomes.push(("intersection".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "intersection");
                outcomes.push(("intersection".to_string(), false));
            }
        }
    }

    // 3. extrude the unit-square footprint by 1 (V = 1)
    {
        let cs = profile(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 1.0, 0.0)]).unwrap();
        let refs: Vec<Option<&Curve>> = cs.iter().map(Some).collect();
        match sweeps::create_linear_sweep(&mut k, &refs, Vec3::new(0.0, 0.0, 1.0), 1.0, true)
            .map_err(|e| e.to_string())
            .and_then(|b| measure_owned(&k, b))
        {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, 1.0) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "extrude");
                outcomes.push(("extrude".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "extrude");
                outcomes.push(("extrude".to_string(), false));
            }
        }
    }

    // 4. revolve the annulus meridian, full turn (V = pi*(1 - 0.4^2)*1)
    {
        let expected = std::f64::consts::PI * (1.0 - 0.4 * 0.4) * 1.0;
        let cs = profile(&[(0.4, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 0.0, 1.0), (0.4, 0.0, 1.0)]).unwrap();
        let refs: Vec<Option<&Curve>> = cs.iter().map(Some).collect();
        match sweeps::create_rotational_sweep(&mut k, &refs, Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 360.0, true)
            .map_err(|e| e.to_string())
            .and_then(|b| measure_owned(&k, b))
        {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, expected) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} expected={expected:.12} faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "revolve");
                outcomes.push(("revolve".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "revolve");
                outcomes.push(("revolve".to_string(), false));
            }
        }
    }

    // 5. fillet one box edge, r = 0.2 (V = 1 - (r^2 - pi*r^2/4))
    {
        let expected = 1.0 - (0.2 * 0.2 - std::f64::consts::PI * 0.2 * 0.2 / 4.0);
        match (|| -> Result<(f64, usize, usize, usize, usize), String> {
            let mut b = primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0)
                .map_err(|e| e.to_string())?;
            let e = pick_one_edge(&b)?;
            fillet_api::fillet_edges(&mut k, &mut b, &[e], 0.2, 1, 1, 1.0)
                .map_err(|err| format!("fillet_edges: {err}"))?;
            measure(&k, &mut b)
        })() {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, expected) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} expected={expected:.12} faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "fillet");
                outcomes.push(("fillet".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "fillet");
                outcomes.push(("fillet".to_string(), false));
            }
        }
    }

    // 6. chamfer: whole-solid op in 0.0.x (no per-edge entry). Reported, not compared.
    {
        match (|| -> Result<(f64, usize, usize, usize, usize), String> {
            let mut b = primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0)
                .map_err(|e| e.to_string())?;
            fillet_api::chamfer_fillet(&mut k, &mut b, 0.2)
                .map_err(|err| format!("chamfer_fillet: {err}"))?;
            measure(&k, &mut b)
        })() {
            Ok((vol, f, e, v, l)) => {
                let pass = vol > 0.0 && vol < 1.0;
                println!("{}  {:<26} volume={vol:.12} faces={f} edges={e} verts={v} loops={l}  [whole-solid op]", if pass { "OK " } else { "FAIL" }, "chamfer(all-edges)");
                println!("            note: no per-edge chamfer API in 0.0.x; not comparable to OCCT single-edge chamfer golden (0.98)");
                outcomes.push(("chamfer(all-edges)".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "chamfer(all-edges)");
                outcomes.push(("chamfer(all-edges)".to_string(), false));
            }
        }
    }

    // 7. bracket plate (80x40x6mm) minus a 6mm through-hole (V = 19200 - pi*9*6)
    {
        let expected = 19200.0 - std::f64::consts::PI * 9.0 * 6.0;
        match (|| -> Result<(f64, usize, usize, usize, usize), String> {
            let cs = profile(&[(0.0, 0.0, 0.0), (80.0, 0.0, 0.0), (80.0, 40.0, 0.0), (0.0, 40.0, 0.0)])?;
            let refs: Vec<Option<&Curve>> = cs.iter().map(Some).collect();
            let plate = sweeps::create_linear_sweep(&mut k, &refs, Vec3::new(0.0, 0.0, 1.0), 6.0, true)
                .map_err(|e| e.to_string())?;
            let hole = primitives::create_cylinder(&mut k, Point3::new(40.0, 20.0, -1.0), 3.0, 8.0)
                .map_err(|e| e.to_string())?;
            let brep = boolean::boolean_difference(&mut k, plate, hole).map_err(|e| e.to_string())?;
            measure_owned(&k, brep)
        })() {
            Ok((vol, f, e, v, l)) => {
                let pass = rel(vol, expected) < REL_BAR;
                println!("{}  {:<26} volume={vol:.12} expected={expected:.12} faces={f} edges={e} verts={v} loops={l}", if pass { "OK " } else { "FAIL" }, "bracket-plate-minus-hole");
                outcomes.push(("bracket-plate-minus-hole".to_string(), pass));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "bracket-plate-minus-hole");
                outcomes.push(("bracket-plate-minus-hole".to_string(), false));
            }
        }
    }

    // 7b. fillet r=2 on the solid result (report only; no closed form).
    {
        match (|| -> Result<(f64, usize, usize, usize, usize), String> {
            let cs = profile(&[(0.0, 0.0, 0.0), (80.0, 0.0, 0.0), (80.0, 40.0, 0.0), (0.0, 40.0, 0.0)])?;
            let refs: Vec<Option<&Curve>> = cs.iter().map(Some).collect();
            let plate = sweeps::create_linear_sweep(&mut k, &refs, Vec3::new(0.0, 0.0, 1.0), 6.0, true)
                .map_err(|e| e.to_string())?;
            let hole = primitives::create_cylinder(&mut k, Point3::new(40.0, 20.0, -1.0), 3.0, 8.0)
                .map_err(|e| e.to_string())?;
            let mut brep = boolean::boolean_difference(&mut k, plate, hole).map_err(|e| e.to_string())?;
            fillet_api::circular_fillet(&mut k, &mut brep, 2.0)
                .map_err(|err| format!("circular_fillet: {err}"))?;
            measure(&k, &mut brep)
        })() {
            Ok((vol, f, e, v, l)) => {
                println!("OK   {:<26} volume={vol:.12} faces={f} edges={e} verts={v} loops={l}", "bracket-plate-fillet-r2");
                println!("            note: fillet r=2 ran on the holed plate; no closed-form gate");
                outcomes.push(("bracket-plate-fillet-r2".to_string(), true));
            }
            Err(err) => {
                println!("FAIL  {:<26} ERROR: {err}", "bracket-plate-fillet-r2");
                outcomes.push(("bracket-plate-fillet-r2".to_string(), false));
            }
        }
    }

    // 8. typed-error probes: Err is success here (no panic on bad input).
    {
        let sphere_err = primitives::create_sphere(&mut k, Point3::new(0.0, 0.0, 0.0), 0.0);
        match sphere_err {
            Ok(_) => {
                println!("FAIL  {:<26} expected Err, got Ok", "err-zero-radius-sphere");
                outcomes.push(("err-zero-radius-sphere".to_string(), false));
            }
            Err(e) => {
                println!("OK    {:<26} Err as expected: {}", "err-zero-radius-sphere", short(&e.to_string()));
                outcomes.push(("err-zero-radius-sphere".to_string(), true));
            }
        }
        let box_err = primitives::create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 0.0, 1.0);
        match box_err {
            Ok(_) => {
                println!("FAIL  {:<26} expected Err, got Ok", "err-zero-dim-box");
                outcomes.push(("err-zero-dim-box".to_string(), false));
            }
            Err(e) => {
                println!("OK    {:<26} Err as expected: {}", "err-zero-dim-box", short(&e.to_string()));
                outcomes.push(("err-zero-dim-box".to_string(), true));
            }
        }
    }

    println!("----------------------------------------------------------------");
    let mut failed = 0;
    for (name, pass) in &outcomes {
        if !pass {
            failed += 1;
            eprintln!("gate failed: {name}");
        }
    }
    if failed == 0 {
        println!("RESULT: ALL PROBE GATES PASSED");
    } else {
        println!("RESULT: {} PROBE GATE(S) FAILED", failed);
        std::process::exit(1);
    }
}
