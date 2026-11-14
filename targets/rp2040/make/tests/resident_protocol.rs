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
 use sha2::{{Digest,Sha256}};
 let(owner,cost)=probe::observe(||image::Initialized::prepare().unwrap());
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 let(offers,cost)=probe::observe(||owner.offers());
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 assert_eq!(offers.len(),5);
 let envelopes:[usize;5]=core::array::from_fn(|i|image::Initialized::available_value_slot_bytes(&owner,i).unwrap());
 assert_eq!(envelopes.into_iter().max(),Some(image::VALUE_SLOT_BYTES));
 assert_eq!(owner.available_value_slot_bytes(5),None);
 println!("actual five-entry prepared value envelopes: {{envelopes:?}}");
 let images:Vec<_>=offers.iter().map(|bytes|{{let bytes=bytes.unwrap();
   conduit_wire::CapabilityOfferImage{{sha256:Sha256::digest(bytes).into(),bytes}}
 }}).collect();
 let mut workspace=vec![0;256*1024];
 let started=std::time::Instant::now();
 let(resolved,cost)=probe::observe(||conduit_wire::resolve_capability_offer_images(&images,&mut workspace,
   conduit_wire::CapabilityOfferImageLimits{{maximum_offers:5,maximum_image_bytes:32*1024,
     maximum_expanded_offer_bytes:256*1024,maximum_total_image_bytes:16*1024,
     maximum_total_expanded_bytes:320*1024}}).unwrap());
 println!("planning-host resolution: {{}} allocated peak bytes excluding preallocated scratch; {{}} allocations; {{:?}} elapsed",cost.peak_bytes,cost.allocations,started.elapsed());
 assert_eq!(resolved.len(),5);
 let(mut expanded,mut retained)=(0,0);
 for (index,bytes) in offers.into_iter().enumerate(){{
   let bytes=bytes.unwrap();
   let document=conduit_wire::DeflatedDocument::parse_image(bytes).unwrap();
   // Planning-host fixture workspace, not an MCU grant or decode operation.
   let mut workspace=vec![0;256*1024];
   let json=document.decode_into(&mut workspace,conduit_wire::DeflatedDocumentLimits{{
     maximum_expanded_bytes:256*1024,maximum_compressed_bytes:32*1024,
   }}).unwrap();
   let offer:conduit_core::CapabilityOffer=serde_json::from_slice(json).unwrap();
   assert_eq!(resolved[index],offer);
   offer.validate_constructed_semantic_contract().unwrap();
   assert_eq!(offer.resource_requirements[0].units,1000);
   assert_eq!(offer.limits.max_active_instances,1);
   expanded+=json.len();retained+=bytes.len();
 }}
 assert!(retained<expanded);
 println!("five resident offers: {{expanded}} raw bytes -> {{retained}} immutable image bytes");
 let(a,b,c,d,e)=owner.into_backs();
 for fuel in [a.unwrap().required_step_fuel(),b.unwrap().required_step_fuel(),c.unwrap().required_step_fuel(),d.unwrap().required_step_fuel(),e.unwrap().required_step_fuel()]{{assert!(fuel<=1002);}}
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
            "conduit_mcu_host_runtime",
            "conduit_kernel",
            "conduit_plot",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
            "conduit_wire",
            "serde_json",
            "sha2",
        ],
    );
}
