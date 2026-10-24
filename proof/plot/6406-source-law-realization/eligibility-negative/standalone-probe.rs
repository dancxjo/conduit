use conduit_core::*;
use conduit_plot::{*, rust_binding::primitive_into_structured};
use conduit_std_host::StdHost;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Ordered = {\n lo: U8\n hi: U8\n where .lo <= .hi\n}\nplot arithmetic (\n >> value: Ordered\n result: Boolean >>\n) = (true)\n".replace("\\n", "\n");
    std::fs::write("probe-source.conduit", &source)?;
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).map_err(|e| format!("check: {e:?}"))?;
    let native = checked.native_types.iter().find(|n| n.name == "Ordered").ok_or("missing Ordered")?;
    println!("checked_type_shape={:?}", native.value_type.shape());
    println!("invariant_count={}", native.invariants.len());
    let u8_type = StructuredInfoType::leaf(kind_id("value/u8")).map_err(|e| format!("leaf: {e:?}"))?;
    let field = |name: &str, n: u8| StructuredFieldValue::new(name, primitive_into_structured(u8_type.clone(), &n).unwrap()).unwrap();
    let invalid = StructuredInfoValue::record(native.value_type.clone(), vec![field("hi", 1),field("lo", 2)]).map_err(|e| format!("record: {e:?}"))?.canonical_bytes().map_err(|e| format!("canonical: {e:?}"))?;
    std::fs::write("invalid-canonical.bin", &invalid)?;
    for (i, law) in native.invariants.iter().enumerate() { println!("invariant_{i}_invalid_output={:?}", law.evaluate(&invalid)); }
    let expanded = expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new()).map_err(|e| format!("expand: {e:?}"))?.expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {return Err("program encoding missing".into());};
    std::fs::write("program.hex", encoded)?;
    let program = PortableExpressionProgram::from_canonical_hex(encoded).map_err(|e| format!("program: {e:?}"))?;
    println!("reference_evaluate_invalid={:?}", program.evaluate(&invalid));
    let mut host = StdHost::new();
    let before = host.advertisement().capabilities.len();
    let result = host.prepare_expression_realizations(&expanded);
    println!("automatic_std_prepare={result:?}");
    println!("offer_count_before={before} offer_count_after={}", host.advertisement().capabilities.len());
    for offer in &host.advertisement().capabilities {if offer.capability_id.as_str().starts_with("std/source-expression/") {println!("automatic_offer={:?}", offer);}}
    println!("proof_class=internal eligibility diagnostic; no ordinary kernel execution or exploit claim");
    Ok(())
}
