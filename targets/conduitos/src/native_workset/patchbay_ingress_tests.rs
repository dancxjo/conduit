//! Authored canonical Fore planning and ordinary fragment-set correspondence.
//! This does not switch the installed Source or claim native scanout proof.
use super::*;

#[test]
fn canonical_patchbay_fore_uses_actual_combined_native_indices() {
    let (ids, offer) = super::super::tests::fixture();
    let wake = super::super::tests::wake(&super::super::inventory());
    let prepared = super::super::prepare(&wake, &ids, &offer, "build").unwrap();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_catalogs(&mut startup, &mut profile).unwrap();
    let mut advertisement = prepared.advertisement().clone();
    advertisement.capabilities.push(super::offer("build"));
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            r#"plot patchbay (
  >> interaction: FaceInteraction...|
) {
  owner: patchbay/accept-interaction
  interaction >> owner.interaction
}"#,
        ),
        &startup,
    )
    .unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "patchbay", &profile).unwrap();
    let canonical = plan(&authored, &advertisement).unwrap();
    let parts = conduit_plan_lowering::fragment_set::lower_local_fragment_set_from_plans(
        &[&prepared.plan().plots[0].plan, &canonical],
        conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE,
        conduit_plan_lowering::fragment_set::FragmentSetBounds {
            fragments: 4,
            nodes: 14,
            cords: 10,
            queue_slots: 10,
            value_bytes: 24_576,
            sign_items: 1024,
            sign_bytes: 1024 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
        },
    )
    .unwrap();
    let admitted = admit(&canonical, &parts.partitions[1]).unwrap().unwrap();
    assert!(admitted.node.0 > 0);
    assert!(admitted.input.cord.0 > 0);
    assert_eq!(admitted.node, parts.partitions[1].nodes[0].node);
    assert_eq!(admitted.input.byte_capacity, BYTES);
    assert_eq!(admitted.input.item_capacity, 1);
    // Admission checks the exact fragment Plan identity before retaining ingress.
    let mut foreign_plan = parts.partitions[1].clone();
    foreign_plan.identity.plan_id = parts.partitions[0].identity.plan_id.clone();
    assert_ne!(
        foreign_plan.identity.plan_id,
        parts.partitions[1].identity.plan_id
    );
    assert!(admit(&canonical, &foreign_plan).is_err());
    // A foreign numeric sink cannot be smuggled in with an authentic Plan.
    let mut foreign = parts;
    foreign.partitions[1].cords[0].spec.sink = conduit_kernel::CordEndpoint::Local {
        node: conduit_kernel::NodeId(0),
        port: conduit_kernel::PortId(0),
    };
    assert!(admit(&canonical, &foreign.partitions[1]).is_err());
}
