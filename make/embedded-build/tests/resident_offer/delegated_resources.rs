//! Exact Core reservation, refusal preservation and kernel-retirement cleanup.
use super::*;

fn observation(host: &c::HostAdvertisement) -> c::ResourceObservation {
    let pool = &host.resources[0];
    c::ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: pool.pool_id.clone(),
        class_id: pool.class_id.clone(),
        health: c::ResourceHealth::Ready,
        unreserved_units: pool.capacity_units,
        utilized_units: 0,
        sign_id: "fixture/resource-ready".into(),
    }
}
fn altered(document: &[u8], entry: &[u8]) -> Vec<u8> {
    let (mut m, projection): (BodyFragmentApprovalMetadata<'_>, _) =
        postcard::take_from_bytes(document).unwrap();
    m.resource_bindings = entry;
    encode_body_fragment_document(&m, projection, 8192).unwrap()
}
pub fn run(a: &crate::support::Assignment) {
    let partition = BodyPlotPlan {
        plot: ResidentPlot::new(
            a.model.source_document_id.clone(),
            a.model.checked_plot_id.clone(),
        ),
        plan: a.model.clone(),
    };
    let body = Body::born_with_plots(
        BodyWorkset::from_plots([partition.plot.clone()]).unwrap(),
        1,
        "fixture/resource-single-body".into(),
    )
    .unwrap();
    let wake = body.wake(1, "fixture/resource-wake".into()).unwrap().1;
    let plan = BodyPlan::seal(&wake, vec![partition]).unwrap();
    let play = BodyPlayIdentity::bind(&plan, 1);
    let scope = PreparedBodyFragment::prepare(&wake, &plan, &play, &a.model.fragments[0]).unwrap();
    let doc = document(scope, a);
    let mut receiver = Receiver::new(scope);
    let placement = &scope.fragment().placements[0];
    let binding = &placement.resources[0];
    receiver.actual.resources.push(c::ResourceOffer {
        pool_id: binding.pool_id.clone(),
        class_id: binding.class_id.clone(),
        capacity_units: binding.units,
        compute: None,
        content: None,
    });
    let prepared = receive_delegated_body_fragment(
        Some(&policy()),
        receiver.context(),
        receiver.command(doc.bytes()),
        profile(a),
    )
    .unwrap();
    let mut owner = c::ResourceAdmissionOwner::new(receiver.actual.clone());
    let observed = observation(owner.host());
    let before = owner.clone();
    assert!(matches!(
        prepared.admit_plain_resources(&mut owner, &[]),
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::MissingObservation
        ))
    ));
    assert_eq!(owner, before);
    let mut stale = observed.clone();
    stale.offer_generation.0 += 1;
    assert!(matches!(
        prepared.admit_plain_resources(&mut owner, &[stale]),
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::StaleObservation
        ))
    ));
    assert_eq!(owner, before);
    let mut unavailable = observed.clone();
    unavailable.health = c::ResourceHealth::Unavailable;
    assert!(matches!(
        prepared.admit_plain_resources(&mut owner, &[unavailable]),
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::Unavailable
        ))
    ));
    assert_eq!(owner, before);
    let mut exhausted = observed.clone();
    exhausted.unreserved_units = binding.units - 1;
    assert!(matches!(
        prepared.admit_plain_resources(&mut owner, &[exhausted]),
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::Overcommitted
        ))
    ));
    assert_eq!(owner, before);
    for field in 0..4 {
        let mut foreign = owner.host().clone();
        match field {
            0 => foreign.offer_generation.0 += 1,
            1 => foreign.profile = "fixture/foreign-profile".into(),
            2 => foreign.boot_id = "fixture/foreign-boot".into(),
            _ => foreign.host_id = "fixture/foreign-host".into(),
        }
        let mut foreign = c::ResourceAdmissionOwner::new(foreign);
        let (_, cost) = crate::probe::observe(|| {
            assert!(matches!(
                prepared.admit_plain_resources(&mut foreign, &[]),
                Err(DelegatedResourceRefusal::Scope)
            ))
        });
        assert_eq!(cost.allocations, 0);
    }
    let mut missing = owner.host().clone();
    missing.resources.clear();
    let mut missing = c::ResourceAdmissionOwner::new(missing);
    let (_, cost) = crate::probe::observe(|| {
        assert!(matches!(
            prepared.admit_plain_resources(&mut missing, &[]),
            Err(DelegatedResourceRefusal::UnsupportedPool)
        ))
    });
    assert_eq!(cost.allocations, 0);
    let mut rich = owner.host().clone();
    rich.resources[0].compute = Some(c::ComputePoolContract {
        service_guarantee: c::ComputeServiceGuarantee::Shared,
        architecture_base_id: "fixture/rich-base".into(),
        architecture_base_kind: c::ArchitectureBaseKind::HostedOs,
        topology_groups: vec![],
        protected_preemption: None,
    });
    let mut rich = c::ResourceAdmissionOwner::new(rich);
    let (_, rejected) = crate::probe::observe(|| {
        assert!(matches!(
            prepared.admit_plain_resources(&mut rich, &[]),
            Err(DelegatedResourceRefusal::UnsupportedPool)
        ))
    });
    assert_eq!(rejected.allocations, 0);
    let (_, cost) = crate::probe::observe(|| {
        prepared
            .admit_plain_resources(&mut owner, core::slice::from_ref(&observed))
            .unwrap();
    });
    let admitted = owner.admissions();
    assert_eq!(admitted.len(), 1);
    assert_eq!(admitted[0].plan_id, scope.plan().plan_id);
    assert_eq!(admitted[0].placement_id, placement.placement_id);
    assert_eq!(admitted[0].items[0].binding, *binding);
    assert_eq!(admitted[0].observation_sign_ids, [observed.sign_id.clone()]);
    let retained = owner.clone();
    assert!(matches!(
        prepared.admit_plain_resources(&mut owner, core::slice::from_ref(&observed)),
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::DuplicatePlanPlacement
        ))
    ));
    assert_eq!(owner, retained);
    let (_, program, _) = crate::support::context();
    let input = crate::support_kernel::frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let mut library = crate::library::Initialized::prepare().unwrap();
    let ((), execution) = crate::probe::observe(|| {
        let (_, back) = library
            .borrow_entry_0(a.selected_scope(AssignedPlayScope::Delegated(&prepared)))
            .unwrap();
        let back = crate::combined::execute_and_retire(back, a, &input, &expected);
        assert!(back.finish_retired().is_ok());
    });
    assert_eq!(execution.allocations, 0);
    assert_eq!(execution.reallocations, 0);
    let released = owner
        .release_for(
            &scope.plan().plan_id,
            &placement.placement_id,
            c::ResourceReleaseCause::Cancelled,
        )
        .unwrap();
    assert_eq!(released.admission, retained.admissions()[0]);
    assert!(owner.admissions().is_empty());
    assert!(library.offers().iter().all(Option::is_some));

    let original = PlainFragmentResources::new(prepared.metadata().resource_bindings)
        .next()
        .unwrap()
        .unwrap();
    let mut entries = vec![Vec::new(), vec![255; 16]];
    for change in 0..4 {
        let mut entry = original;
        match change {
            0 => entry.units += 1,
            1 => entry.node = u16::MAX,
            2 => entry.resource += 1,
            _ => entry.placement = "",
        }
        entries.push(postcard::to_allocvec(&entry).unwrap());
    }
    let mut duplicate = prepared.metadata().resource_bindings.to_vec();
    duplicate.extend_from_slice(prepared.metadata().resource_bindings);
    entries.push(duplicate);
    for entries in entries {
        let changed = altered(doc.bytes(), &entries);
        let (_, refused) = crate::probe::observe(|| {
            assert!(matches!(
                receive_delegated_body_fragment(
                    Some(&policy()),
                    receiver.context(),
                    receiver.command(&changed),
                    profile(a)
                ),
                Err(DelegatedFragmentRefusal::Resources(
                    DelegatedResourceRefusal::Projection
                ))
            ))
        });
        assert_eq!(refused.allocations, 0);
        assert_eq!(refused.reallocations, 0);
    }
    let mut unsupported = original;
    unsupported.pool = "fixture/not-installed";
    let unsupported = postcard::to_allocvec(&unsupported).unwrap();
    let changed = altered(doc.bytes(), &unsupported);
    let prepared = receive_delegated_body_fragment(
        Some(&policy()),
        receiver.context(),
        receiver.command(&changed),
        profile(a),
    )
    .unwrap();
    assert!(matches!(
        prepared.admit_plain_resources(&mut owner, &[observed]),
        Err(DelegatedResourceRefusal::UnsupportedPool)
    ));
    assert!(owner.admissions().is_empty());
    println!("exact original Body Plan/placement/binding reserved through Core; preparation heap peak {} bytes ({} allocations); allocation-free execution/retirement; missing/stale/unavailable/exhausted resources preserve old reservations",cost.peak_bytes,cost.allocations);
}

pub fn batch(
    a: PreparedBodyFragment<'_>,
    b: PreparedBodyFragment<'_>,
    first: &crate::support::Assignment,
    second: &crate::support::Assignment,
) {
    prepared_storage::run(a, b, first, second);
    let docs = [document(a, first), document(b, second)];
    let bindings = [
        a.fragment().placements[0].resources[0].clone(),
        b.fragment().placements[0].resources[0].clone(),
    ];
    assert_eq!(bindings[0].pool_id, bindings[1].pool_id);
    assert_ne!(
        a.fragment().placements[0].placement_id,
        b.fragment().placements[0].placement_id
    );
    let mut receiver = Receiver::new(a);
    let capacity = bindings[0].units + bindings[1].units + 1;
    receiver.actual.resources.push(c::ResourceOffer {
        pool_id: bindings[0].pool_id.clone(),
        class_id: bindings[0].class_id.clone(),
        capacity_units: capacity,
        compute: None,
        content: None,
    });
    let left = receive_delegated_body_fragment(
        Some(&policy()),
        receiver.context(),
        receiver.command(docs[0].bytes()),
        profile(first),
    )
    .unwrap();
    let right = receive_delegated_body_fragment(
        Some(&policy()),
        receiver.context(),
        receiver.command(docs[1].bytes()),
        profile(second),
    )
    .unwrap();
    let mut owner = c::ResourceAdmissionOwner::new(receiver.actual.clone());
    let observed = observation(owner.host());
    let mut item = owner.host().resources[0].clone();
    item.capacity_units = 1;
    owner
        .admit(
            c::ResourceAdmissionRequest {
                plan_id: "fixture/previous-plan".into(),
                placement_id: "fixture/previous-placement".into(),
                host_id: owner.host().host_id.clone(),
                boot_id: owner.host().boot_id.clone(),
                offer_generation: owner.host().offer_generation,
                items: vec![c::ResourceAdmissionItem {
                    requirement: c::ResourceRequirement {
                        class_id: item.class_id.clone(),
                        units: 1,
                        protected_role: None,
                        compute: None,
                        content: None,
                    },
                    binding: c::ResourceBinding {
                        pool_id: item.pool_id,
                        class_id: item.class_id,
                        units: 1,
                        protected: None,
                        compute: None,
                        content: None,
                    },
                }],
            },
            core::slice::from_ref(&observed),
        )
        .unwrap();
    let before = owner.clone();
    assert!(matches!(
        left.admit_plain_resources(&mut owner, core::slice::from_ref(&observed)),
        Err(DelegatedResourceRefusal::Scope)
    ));
    assert!(matches!(
        admit_delegated_plain_resources(
            &[&left, &left],
            &mut owner,
            core::slice::from_ref(&observed)
        ),
        Err(DelegatedResourceRefusal::Scope)
    ));
    assert_eq!(owner, before);
    let mut exhausted = observed.clone();
    exhausted.unreserved_units -= 1;
    assert!(matches!(
        admit_delegated_plain_resources(&[&right, &left], &mut owner, &[exhausted]),
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::Overcommitted
        ))
    ));
    assert_eq!(owner, before);
    let bridge_bound = delegated_plain_resource_preparation_bytes(
        &[&left, &right],
        &owner,
        core::slice::from_ref(&observed),
    )
    .unwrap();
    let (_, preparation) = crate::probe::observe(|| {
        admit_delegated_plain_resources(
            &[&left, &right],
            &mut owner,
            core::slice::from_ref(&observed),
        )
        .unwrap();
    });
    // Only Core grows the preexisting reservation ledger. The bridge creates
    // its complete request table once, without per-fragment reallocations.
    assert_eq!(preparation.reallocations, 1);
    assert!(preparation.requested_bytes <= bridge_bound);
    let admitted = &owner.admissions()[before.admissions().len()..];
    assert_eq!(admitted.len(), 2);
    assert_eq!(
        admitted[0].placement_id,
        a.fragment().placements[0].placement_id
    );
    assert_eq!(
        admitted[1].placement_id,
        b.fragment().placements[0].placement_id
    );
    let mut library = crate::library::Initialized::prepare().unwrap();
    let (_, p, _) = crate::support::context();
    let input = crate::support_kernel::frame(&p);
    let output = p.evaluate(&input).unwrap();
    let (_, p, _) = crate::support::context_for(c::PortTemporal::Value);
    let input2 = crate::support_kernel::frame(&p);
    let output2 = p.evaluate(&input2).unwrap();
    let ((), cost) = crate::probe::observe(|| {
        let (x, y) = library
            .borrow_selected([
                Some(first.selected_scope(AssignedPlayScope::Delegated(&left))),
                Some(second.selected_scope(AssignedPlayScope::Delegated(&right))),
            ])
            .unwrap();
        let x = crate::combined::execute_and_retire(x.unwrap().1, first, &input, &output);
        let y = crate::combined::execute_and_retire(y.unwrap().1, second, &input2, &output2);
        assert!(x.finish_retired().is_ok());
        assert!(y.finish_retired().is_ok());
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    for scope in [a, b] {
        owner
            .release_for(
                &scope.plan().plan_id,
                &scope.fragment().placements[0].placement_id,
                c::ResourceReleaseCause::Cancelled,
            )
            .unwrap();
    }
    assert_eq!(owner, before);
    assert!(library.offers().iter().all(Option::is_some));
    println!("two exact Body fragments reserve atomically: late shortage preserves prior reservation; both typed kernels retire and release only their original scopes; preparation heap peak {} bytes ({} allocations)", preparation.peak_bytes, preparation.allocations);
}

#[path = "prepared_storage.rs"]
mod prepared_storage;
