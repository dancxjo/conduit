//! Malformed internal producer graphs must refuse before material expansion.
use super::*;
fn base() -> NativeLawWitness {
    let doc = crate::check_syntax_document(
        &crate::parse_syntax_document("type Leaf = {\n value: U64\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    doc.native_law_witness("Leaf").unwrap().clone()
}
fn parent(child: &NativeLawWitness, fanout: usize) -> NativeLawWitness {
    NativeLawWitness {
        checked: child.checked.clone(),
        module: Shared::new(Module {
            source: child.module.source.clone(),
            families: Vec::new(),
            imports: (0..fanout)
                .map(|n| (alloc::format!("alias{n}"), child.clone()))
                .collect(),
        }),
    }
}
#[test]
fn export_checks_chain_depth_before_expanding_any_material() {
    let mut witness = base();
    for _ in 0..MAXIMUM_DEPTH {
        witness = parent(&witness, 1);
    }
    assert!(witness.material().is_ok());
    witness = parent(&witness, 1);
    assert!(matches!(
        witness.material(),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}
#[test]
fn shared_dag_is_charged_for_every_exported_occurrence() {
    let mut witness = base();
    for _ in 0..7 {
        witness = parent(&witness, 2);
    }
    assert!(witness.material().is_ok()); //255 expanded modules, not8 Shared nodes.
    witness = parent(&witness, 2);
    assert!(matches!(
        witness.material(),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}
#[test]
fn aggregate_source_bytes_refuse_even_when_each_module_is_individually_bounded() {
    let mut witness = base();
    witness.module = Shared::new(Module {
        source: " ".repeat(MAXIMUM_BYTES / 2 + 1),
        families: Vec::new(),
        imports: Vec::new(),
    });
    assert!(witness.material().is_ok());
    witness = parent(&witness, 1);
    assert!(matches!(
        witness.material(),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}
