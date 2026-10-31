//! Installed Make custody preserves the existing evaluator, not received proof.
use conduit_plot::*;
use sha2::{Digest, Sha256};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program(expression: &str) -> PortableExpressionProgram {
    let source = format!("plot x (\n >> value: U64\n result: U64 >>\n) = ({expression})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "x", &ProfileCatalog::new()).unwrap();
    let conduit_core::ConfigurationValue::Text(text) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(text).unwrap()
}
fn material(p: &PortableExpressionProgram) -> (&'static [u8], [u8; 32], (u32, u32)) {
    let m = MakeExpressionMaterial::export(p).unwrap();
    (
        Box::leak(m.bytes().to_vec().into_boxed_slice()),
        Sha256::digest(p.canonical_bytes().unwrap()).into(),
        (
            p.maximum_prepared_input_bytes().unwrap(),
            p.maximum_prepared_output_bytes().unwrap(),
        ),
    )
}
#[test]
fn checked_make_keeps_arithmetic_lazy_failures_and_independent_outputs_allocation_free() {
    for expression in [
        ". + 1",
        "100 / .",
        ". == 0 ? 42 : (100 / .)",
        ". == 0 ? (. + .) : .",
    ] {
        let p = program(expression);
        let (bytes, digest, bounds) = material(&p);
        // SAFETY: this fixture exported the complete ordinarily checked program
        // immediately above; a received digest is not used to establish origin.
        let mut a = unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make_with_bounds(bytes, digest, bounds)
        }
        .unwrap();
        let mut b = unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make_with_bounds(bytes, digest, bounds)
        }
        .unwrap();
        let mut ordinary = PreparedPortableExpressionEvaluator::new_compact(&p).unwrap();
        for n in [0u64, 1, 2, u64::MAX] {
            let input = n.to_le_bytes();
            let expected = ordinary.evaluate(&input).map(|v| v.to_vec());
            let (_, memory) = allocation_probe::observe(|| {
                for _ in 0..8 {
                    match &expected {
                        Ok(v) => assert_eq!(a.evaluate(&input).unwrap(), v),
                        Err(e) => assert_eq!(&a.evaluate(&input).unwrap_err(), e),
                    }
                }
            });
            assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        }
        let input = 1u64.to_le_bytes();
        let held = b.evaluate(&input).unwrap();
        let copy = held.to_vec();
        let _ = a.evaluate(&2u64.to_le_bytes());
        assert_eq!(held, copy);
        assert_eq!(
            a.evaluate(&[0]).unwrap_err(),
            ordinary.evaluate(&[0]).unwrap_err()
        );
    }
}
#[test]
fn malformed_material_and_wrong_bound_pins_refuse_before_allocation() {
    let p = program(". + 1");
    let (bytes, digest, bounds) = material(&p);
    for end in 0..bytes.len() {
        let (result, memory) = allocation_probe::observe(|| unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make_with_bounds(
                &bytes[..end],
                digest,
                bounds,
            )
        });
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    for wrong in [
        (bounds.0 + 1, bounds.1),
        (bounds.0, bounds.1 + 1),
        (65537, bounds.1),
    ] {
        let (result, memory) = allocation_probe::observe(|| unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make_with_bounds(bytes, digest, wrong)
        });
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    assert!(
        unsafe { PreparedPortableExpressionEvaluator::from_linked_make(bytes, [0; 32]) }.is_err()
    );
    let h = b"conduit.make.compact-expression.v1\0".len() + 40;
    let root = h + 8;
    let records = root + 9;
    for (offset, change) in [
        (0, vec![0]),
        (h + 1, 65537u32.to_le_bytes().to_vec()),
        (h + 6, vec![255]),
        (root, vec![255]),
        (root + 1, 999u32.to_le_bytes().to_vec()),
        (records + 4, 999u32.to_le_bytes().to_vec()),
        (records + 24 + 8, u32::MAX.to_le_bytes().to_vec()),
    ] {
        let mut bad = bytes.to_vec();
        bad[offset..offset + change.len()].copy_from_slice(&change);
        let bad = Box::leak(bad.into_boxed_slice());
        let (result, memory) = allocation_probe::observe(|| unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make(bad, digest)
        });
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}
