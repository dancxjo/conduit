use conduit_planner::proof::resource_frame::*;
#[test]
fn unchanged_form_plans_copy_and_shared_residence_and_refuses_foreign_owner() {
    let copy = frame_resource_plan(true, false).unwrap();
    let shared = frame_resource_plan(false, false).unwrap();
    assert_eq!(copy.plan.checked_form_id, shared.plan.checked_form_id);
    assert_eq!(copy.plan.expanded_form_id, shared.plan.expanded_form_id);
    assert_ne!(copy.plan.plan_id, shared.plan.plan_id);
    assert_eq!(copy.plan.fragments[0].connections.len(), 3);
    for (a, b) in copy.plan.fragments[0]
        .connections
        .iter()
        .zip(&shared.plan.fragments[0].connections)
    {
        assert_eq!(a.value_kind, b.value_kind);
        assert_eq!(a.source_port_id, b.source_port_id);
        assert_eq!(a.sink_port_id, b.sink_port_id);
    }
    assert!(frame_resource_plan(false, true)
        .err()
        .unwrap()
        .contains("ForeignResidence"));
}

#[test]
fn resource_generation_bounds_and_residence_are_sealed_into_plan_identity() {
    use conduit_core::*;
    let proof = frame_resource_plan(false, false).unwrap();
    let original = proof.plan.clone();
    let identity = FormIdentity {
        source_document_id: original.source_document_id.clone(),
        checked_form_id: original.checked_form_id.clone(),
        expanded_form_id: original.expanded_form_id.clone(),
    };
    for change in [
        |c: &mut ResourceContentOffer| c.contract.version = [9; 32],
        |c: &mut ResourceContentOffer| c.contract.reader_leases = 2,
        |c: &mut ResourceContentOffer| c.contract.retention = ResourceRetention::Boot,
        |c: &mut ResourceContentOffer| c.residence_profile = kind_id("different/residence@1"),
        |c: &mut ResourceContentOffer| c.owner_boot = BootId::from("boot/replacement"),
    ] {
        let mut fragments = original.fragments.clone();
        let content = fragments[0]
            .placements
            .iter_mut()
            .flat_map(|p| &mut p.resources)
            .find_map(|r| r.content.as_mut())
            .unwrap();
        change(content);
        let changed = seal_plan(identity.clone(), fragments);
        assert_ne!(changed.plan_id, original.plan_id);
    }
    assert_eq!(proof.plan, original);
}

#[test]
fn resource_owner_refuses_a_second_writer_before_displacing_existing_admission() {
    use conduit_core::*;
    let proof = frame_resource_plan(false, false).unwrap();
    let compose = proof.plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == "frame/compose")
        .unwrap();
    let observations = proof
        .host
        .resources
        .iter()
        .map(|r| ResourceObservation {
            host_id: proof.host.host_id.clone(),
            boot_id: proof.host.boot_id.clone(),
            offer_generation: proof.host.offer_generation,
            pool_id: r.pool_id.clone(),
            class_id: r.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: r.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("ready/{}", r.pool_id.as_str())),
        })
        .collect::<Vec<_>>();
    let mut owner = ResourceAdmissionOwner::new(proof.host);
    owner
        .admit_planned_placement(proof.plan.plan_id.clone(), compose, &observations)
        .unwrap();
    let before = owner.clone();
    assert_eq!(
        owner.admit_planned_placement(PlanId::from("another/plan"), compose, &observations),
        Err(ResourceAdmissionRefusal::InvalidBinding)
    );
    assert_eq!(owner, before);
}
