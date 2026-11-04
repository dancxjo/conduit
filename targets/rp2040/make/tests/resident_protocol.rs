//! Exact Make-selected installation closure, compiled natively and without std.
use conduit_host_make::*;
#[path = "../../../../architecture/plot/tests/common/compiled_rust.rs"]
#[allow(dead_code)]
mod compiled_rust;

#[test]
fn checked_equipment_emits_all_five_initialized_protocol_backs() {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let configuration = parse_host_configuration_conduit(include_str!(
        "../../profiles/pico-w-house-green.host.conduit"
    ))
    .unwrap();
    let checked = check_host_configuration(
        configuration,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap();
    let source = conduit_host_rp2040::checked_ht16k33_resident_library(&checked).unwrap();
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../architecture/plot/tests/common/allocation_probe.rs");
    let exercise = format!(
        r#"#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
fn main(){{
 let(owner,cost)=probe::observe(||image::Initialized::prepare().unwrap());
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 let(offers,cost)=probe::observe(||owner.offers());
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 assert_eq!(offers.len(),5);
 for json in offers{{
   let offer:conduit_core::CapabilityOffer=serde_json::from_slice(json).unwrap();
   assert_eq!(offer.resource_requirements[0].units,1000);
   assert_eq!(offer.limits.max_active_instances,1);
 }}
 let(a,b,c,d,e)=owner.into_backs();
 for fuel in [a.required_step_fuel(),b.required_step_fuel(),c.required_step_fuel(),d.required_step_fuel(),e.required_step_fuel()]{{assert!(fuel<=1002);}}
}}
"#,
        probe = probe.to_str().unwrap()
    );
    compiled_rust::verify_with_libraries(
        &format!("mod image{{\n{source}\n}}"),
        &exercise,
        "rp2040-resident-protocol",
        &[
            "conduit_core",
            "conduit_plot",
            "conduit_semantic_catalog",
            "serde_json",
        ],
    );
}
