//! Exercise actual firmware inventory custody with the checked emitted library.
use conduit_host_make::*;
#[path = "../../../../architecture/plot/tests/common/compiled_rust.rs"]
#[allow(dead_code)]
mod compiled_rust;

#[test]
fn retained_inventory_is_exclusive_and_returns_when_async_work_is_cancelled() {
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
    let library = conduit_host_rp2040::checked_ht16k33_resident_library(&checked).unwrap();
    let source = include_str!("../../firmware/host/src/resident.rs").replace(
        "include!(concat!(env!(\"OUT_DIR\"), \"/resident.rs\"));",
        &library,
    );
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../architecture/plot/tests/common/allocation_probe.rs");
    let exercise = format!(
        r#"
#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
fn main(){{
 use core::future::Future;
 critical_section::with(|_|{{}});
 assert!(matches!(inventory::inventory(),Err(inventory::InventoryRefusal::NotInitialized)));
 let((),cost)=probe::observe(inventory::initialize);
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 let(count,bytes,refusal)=inventory::summary();
 assert_eq!(count,5);assert_eq!(refusal,None);assert!(bytes>16*1024);
 assert_eq!(core::mem::size_of::<inventory::InventoryLease>(),core::mem::size_of::<usize>());
 let((),cost)=probe::observe(||{{
   let mut lease=inventory::inventory().unwrap();
   let address=(&*lease as *const inventory::generated::Initialized)as usize;
   let _: &mut inventory::generated::Initialized=&mut lease;
   assert!(core::mem::size_of::<inventory::generated::BorrowedBack<'_>>()<=2*core::mem::size_of::<usize>());
   let loans=lease.borrow_selected_array(core::array::from_fn(|_|None)).unwrap();
   assert!(loans.into_iter().all(|loan|loan.is_none()));
   assert!(matches!(inventory::inventory(),Err(inventory::InventoryRefusal::Busy)));
   assert_eq!(inventory::summary().0,5);
   assert_eq!(lease.offers().into_iter().flatten().count(),5);
   drop(lease);
   let lease=inventory::inventory().unwrap();
   assert_eq!((&*lease as *const inventory::generated::Initialized)as usize,address);
 }});
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 let future=async{{
   let _lease=inventory::inventory().unwrap();
   core::future::pending::<()>().await;
 }};
 let future_bytes=core::mem::size_of_val(&future);
 assert!(future_bytes<=2*core::mem::size_of::<usize>());
 let mut future=Box::pin(future);
 let((),cost)=probe::observe(||{{
   let mut context=core::task::Context::from_waker(core::task::Waker::noop());
   assert!(future.as_mut().poll(&mut context).is_pending());
 }});
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 assert!(matches!(inventory::inventory(),Err(inventory::InventoryRefusal::Busy)));
 let((),cost)=probe::observe(||drop(future));
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 assert_eq!(inventory::inventory().unwrap().offers().into_iter().flatten().count(),5);
 println!("retained inventory {{bytes}} bytes; lease {{}} bytes; cancelled future {{future_bytes}} bytes",core::mem::size_of::<inventory::InventoryLease>());
}}
"#,
        probe = probe.to_str().unwrap()
    );
    compiled_rust::verify_with_libraries(
        &format!("mod inventory{{\n{source}\n}}"),
        &exercise,
        "resident-inventory-custody",
        &[
            "conduit_core",
            "conduit_mcu_host_runtime",
            "conduit_kernel",
            "conduit_plot",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
            "critical_section",
            "static_cell",
        ],
    );
}
