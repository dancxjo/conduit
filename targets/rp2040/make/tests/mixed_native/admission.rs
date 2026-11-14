//! Original signed workset plus the actual Root atomic admission path.
use super::*;
use crate::mixed_workset::{self, Policy};
use conduit_body as b;
#[path = "issuance.rs"]
mod issuance;
#[path = "observations.rs"]
mod observations;
#[path = "receiver.rs"]
mod receiver;
#[path = "start.rs"]
mod start;
#[path = "supervision.rs"]
mod supervision;

pub fn run(
    context: &body_scope::Context,
    fragment: &PlanFragment,
    contract: &I2cContract,
    authority: &BaseCapabilityScope,
    image: &conduit_embedded_build::PreparedResidentExpression,
    maxima: AssignedPlanMaxima,
    values: (&[u8], &[u8]),
) {
    let AssignedPlayScope::Body(scope) = context.scope(fragment) else {
        unreachable!()
    };
    let mut receiver = receiver::Receiver::new(scope);
    receiver.actual.resources = vec![
        resource_offer("fixture/private-work", COOPERATIVE_STEP_WORK_CLASS_ID, 1000),
        resource_offer(authority.resource_pool_id.as_str(), I2C_ATTACHMENT_CLASS, 1),
        resource_offer("fixture/decoy-work", COOPERATIVE_STEP_WORK_CLASS_ID, 1000),
        resource_offer("fixture/decoy-bus", I2C_ATTACHMENT_CLASS, 1),
    ];
    let document = conduit_embedded_build::generate_prepared_body_workset(
        scope,
        &receiver.actual,
        4096,
        |local| {
            let resident = local
                .fragment()
                .placements
                .iter()
                .find(|p| p.artifact_id == *image.artifact_id())
                .unwrap();
            let bound = image
                .bind(
                    local.plot_plan(),
                    &local.fragment().fragment_id,
                    &resident.placement_id,
                )
                .unwrap();
            Ok((
                vec![bound],
                conduit_embedded_build::BodyFragmentProjectionLimits {
                    image: conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
                    computations: conduit_embedded_build::EmbeddedComputationBounds {
                        maximum_program_bytes: 32768,
                        maximum_total_program_bytes: 32768,
                    },
                    assigned: maxima,
                    maximum_fore_ports: 2,
                    maximum_document_bytes: 4096,
                },
            ))
        },
    )
    .unwrap();
    let mut retained = b::HostCommandInput::<4096>::new();
    retained.writable().unwrap()[..document.bytes().len()].copy_from_slice(document.bytes());
    retained
        .retain(
            receiver
                .command_with_contract(document.bytes(), b::HOST_PREPARED_WORKSET_CONTRACT)
                .into_retained_stamp()
                .unwrap(),
        )
        .unwrap();
    let preparation = b::HostPreparationDelegation {
        authorizer: b::HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 4096,
        maximum_runtime_state_bytes: 16384,
    };
    let execution = b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: None,
    };
    let policy = Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        work_pool: "fixture/private-work",
        maxima,
        maximum_storage_bytes: 16384,
    };
    let mut owner = ResourceAdmissionOwner::new(receiver.actual.clone());
    let before = owner.clone();
    let observations: Vec<_> = owner
        .host()
        .resources
        .iter()
        .enumerate()
        .map(|(index, r)| ResourceObservation {
            host_id: receiver.actual.host_id.clone(),
            boot_id: receiver.actual.boot_id.clone(),
            pool_id: r.pool_id.clone(),
            class_id: r.class_id.clone(),
            offer_generation: receiver.actual.offer_generation,
            health: ResourceHealth::Ready,
            unreserved_units: r.capacity_units,
            utilized_units: 0,
            sign_id: format!("fixture/current/{index}").into(),
        })
        .collect();
    let effects = core::cell::Cell::new(0);
    let mut slot = ready_slot(authority, &effects);
    let mut library = crate::resident::generated::Initialized::prepare().unwrap();
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    let issued = core::cell::Cell::new(0);
    let (route, route_cost) = crate::probe::observe(|| {
        crate::execution_selection::select::<4096, 1>(
            &retained,
            receiver.context(),
            Some(&preparation),
            Some(&execution),
        )
    });
    assert_eq!(
        route.unwrap(),
        crate::execution_selection::Profile::MixedI2c
    );
    assert_eq!((route_cost.allocations, route_cost.reallocations), (0, 0));
    let (route, route_cost) = crate::probe::observe(|| {
        crate::execution_selection::select::<4096, 1>(
            &retained,
            receiver.context(),
            Some(&preparation),
            None,
        )
    });
    assert!(matches!(
        route,
        Err(crate::execution_selection::Refusal::Execution(_))
    ));
    assert_eq!((route_cost.allocations, route_cost.reallocations), (0, 0));
    let mut late = receiver.context();
    late.now_millis = 20;
    let (route, cost) = crate::probe::observe(|| {
        crate::execution_selection::select::<4096, 1>(
            &retained,
            late,
            Some(&preparation),
            Some(&execution),
        )
    });
    assert!(matches!(
        route,
        Err(crate::execution_selection::Refusal::Retention(_))
    ));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let issuer_checked = core::cell::Cell::new(false);
    let issuer = core::cell::RefCell::new(crate::i2c_issuance::Issuer::new());
    let issue = |approved: &b::DelegatedBodyFragment<'_>, native: &ReadyI2cBase<Provider<'_>>| {
        issued.set(issued.get() + 1);
        assert_eq!(approved.metadata().body_plan, context.plan_id().as_str());
        assert_eq!(approved.metadata().body_play, context.play_id().as_str());
        assert_eq!(
            native.native_identity().resource_pool_id,
            authority.resource_pool_id
        );
        if crate::config::I2C_CALL_MAXIMUM_OPERATIONS != 0
            && crate::config::EXECUTION_DELEGATION.is_some()
            && !issuer_checked.replace(true)
        {
            issuance::check(receiver.context(), approved, native);
        }
        if crate::config::I2C_CALL_MAXIMUM_OPERATIONS != 0
            && crate::config::EXECUTION_DELEGATION.is_some()
        {
            issuer.borrow_mut().authorize(
                receiver.context(),
                native,
                policy.maximum_storage_bytes,
            )?;
        }
        issuer.borrow_mut().issue(
            receiver.context(),
            approved,
            native,
            policy.maximum_storage_bytes,
        )
    };
    macro_rules! prepare {
        ($policy:expr,$context:expr) => {{
            mixed_workset::prepare_retained_with_issuer(
                &mut library,
                &mut slot,
                contract,
                $policy,
                $context,
                &retained,
                issue,
            )
        }};
    }
    let mut expired = receiver.context();
    expired.now_millis = 20;
    for (policy, current) in [
        (
            Policy {
                execution: None,
                ..policy
            },
            receiver.context(),
        ),
        (
            Policy {
                active_worksets: 1,
                ..policy
            },
            receiver.context(),
        ),
        (policy, expired),
    ] {
        let issued_before = issued.get();
        let (result, cost) = crate::probe::observe(|| {
            mixed_workset::prepare_retained_with_issuer(
                &mut library,
                &mut slot,
                contract,
                policy,
                current,
                &retained,
                issue,
            )
        });
        assert_eq!(issued.get(), issued_before);
        assert!(result.is_err());
        drop(result);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert!(slot.is_some());
        assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
        assert_eq!(owner, before);
        assert_eq!(effects.get(), 0);
    }
    // Even a correctly attributed owner cannot substitute another available
    // pool for the local resident/native owners while keeping numeric slots.
    let original_set = b::receive_delegated_body_workset::<1>(
        Some(&preparation),
        receiver.context(),
        receiver.command_with_contract(document.bytes(), b::HOST_PREPARED_WORKSET_CONTRACT),
    )
    .unwrap();
    let (metadata, projection) =
        b::decode_body_fragment_document(original_set.document(0).unwrap()).unwrap();
    for native in [false, true] {
        let mut bindings = Vec::new();
        for entry in b::PlainFragmentResources::new(metadata.resource_bindings) {
            let mut entry = entry.unwrap();
            if native && entry.class == I2C_ATTACHMENT_CLASS {
                entry.pool = "fixture/decoy-bus";
            }
            if !native && entry.class == COOPERATIVE_STEP_WORK_CLASS_ID {
                entry.pool = "fixture/decoy-work";
            }
            entry.append_to(&mut bindings).unwrap();
        }
        let mut altered = metadata;
        altered.resource_bindings = &bindings;
        let altered = b::encode_body_fragment_document(&altered, projection, 4096).unwrap();
        let altered = b::encode_body_workset_document(&[&altered], 4096).unwrap();
        let mut input = b::HostCommandInput::<4096>::new();
        input.writable().unwrap()[..altered.len()].copy_from_slice(&altered);
        input
            .retain(
                receiver
                    .command_with_contract(&altered, b::HOST_PREPARED_WORKSET_CONTRACT)
                    .into_retained_stamp()
                    .unwrap(),
            )
            .unwrap();
        let issued_before = issued.get();
        let (result, cost) = crate::probe::observe(|| {
            mixed_workset::prepare_retained_with_issuer(
                &mut library,
                &mut slot,
                contract,
                policy,
                receiver.context(),
                &input,
                issue,
            )
        });
        assert_eq!(issued.get(), issued_before);
        assert!(matches!(
            result,
            Err(mixed_workset::Refusal::Resources(
                b::DelegatedResourceRefusal::Scope
            ))
        ));
        drop(result);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert_eq!(owner, before);
        assert!(slot.is_some());
        assert_eq!(effects.get(), 0);
        assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    }
    let (refused, cost) = crate::probe::observe(|| {
        mixed_workset::prepare_retained_with_issuer(
            &mut library,
            &mut slot,
            contract,
            policy,
            receiver.context(),
            &retained,
            |_, _| Err(crate::native_preparation::Refusal::Scope),
        )
    });
    assert!(matches!(
        refused,
        Err(mixed_workset::Refusal::Native(
            crate::native_preparation::Refusal::Scope
        ))
    ));
    drop(refused);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(slot.is_some());
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    if crate::config::I2C_CALL_MAXIMUM_OPERATIONS == 0
        || crate::config::EXECUTION_DELEGATION.is_none()
    {
        let before_issued = issued.get();
        let (result, cost) = crate::probe::observe(|| prepare!(policy, receiver.context()));
        assert!(matches!(
            result,
            Err(mixed_workset::Refusal::Native(
                crate::native_preparation::Refusal::Scope
            ))
        ));
        drop(result);
        assert_eq!(issued.get(), before_issued + 1);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert!(slot.is_some());
        assert_eq!(owner, before);
        assert_eq!(effects.get(), 0);
        assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
        println!("actual Root native issuance disabled: complete signed approval cannot create possession; zero allocations, reservations, provider transfers or effects");
        return;
    }
    let prepared = prepare!(policy, receiver.context()).unwrap();
    let kernel_bytes = prepared.kernel_storage_bytes();
    let total = prepared.preparation_bytes(&owner, &observations).unwrap();
    assert!(total <= policy.maximum_storage_bytes);
    let unselected_extra = if crate::resident::generated::ENTRY_COUNT == 5 {
        let selected = image
            .maximum_input_bytes()
            .max(image.maximum_output_bytes())
            .max(conduit_i2c_transaction::contract::I2C_VALUE_SLOT_BYTES as u32);
        let global = crate::resident::generated::VALUE_SLOT_BYTES as u32;
        let selected_bytes = HostedValueStore::canonical_storage_bytes([selected; 3]).unwrap();
        let global_bytes = HostedValueStore::canonical_storage_bytes([global; 3]).unwrap();
        let unselected_extra = global_bytes - selected_bytes;
        assert!(unselected_extra > 0);
        println!("actual five-entry library: {total} B selected preparation versus {} B if unrelated widest entry were charged; {unselected_extra} B avoided",total+unselected_extra);
        unselected_extra
    } else {
        0
    };

    assert!(matches!(
        prepared.admit_and_bind(&mut owner, &[]),
        Err(mixed_workset::Refusal::Resources(
            b::DelegatedResourceRefusal::Admission(ResourceAdmissionRefusal::MissingObservation)
        ))
    ));
    assert!(slot.is_some());
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    if unselected_extra > 0 {
        // Test the actual selected preparation boundary, rather than a stale
        // fixed ceiling that later general memory reductions may undercut.
        let exact_policy = Policy {
            maximum_storage_bytes: total,
            ..policy
        };
        let exact = prepare!(exact_policy, receiver.context()).unwrap();
        assert!(
            exact.preparation_bytes(&owner, &observations).unwrap()
                <= exact_policy.maximum_storage_bytes
        );
        assert!(total + unselected_extra > exact_policy.maximum_storage_bytes);
        drop(exact);
        assert_eq!(owner, before);
        assert_eq!(effects.get(), 0);
        assert!(slot.is_some());
    }
    let prepared = prepare!(policy, receiver.context()).unwrap();
    let (result, cost) =
        crate::probe::observe_refusing(1, || prepared.admit_and_bind(&mut owner, &observations));
    assert!(matches!(
        result,
        Err(mixed_workset::Refusal::Resources(
            b::DelegatedResourceRefusal::Allocation
                | b::DelegatedResourceRefusal::Admission(ResourceAdmissionRefusal::Allocation)
        ))
    ));
    drop(result);
    assert_eq!(cost.refused_allocations, 1);
    assert_eq!(owner, before);
    assert!(slot.is_some());
    assert_eq!(effects.get(), 0);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let mut low = observations.clone();
    low[1].unreserved_units = 0;
    assert!(matches!(
        prepare!(policy, receiver.context())
            .unwrap()
            .admit_and_bind(&mut owner, &low),
        Err(mixed_workset::Refusal::Resources(
            b::DelegatedResourceRefusal::Admission(ResourceAdmissionRefusal::Overcommitted)
        ))
    ));
    assert_eq!(owner, before);
    assert!(slot.is_some());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    assert_eq!(effects.get(), 0);
    assert!(matches!(
        prepare!(
            Policy {
                maximum_storage_bytes: total - 1,
                ..policy
            },
            receiver.context()
        )
        .unwrap()
        .admit_and_bind(&mut owner, &observations),
        Err(mixed_workset::Refusal::Bounds)
    ));
    assert_eq!(owner, before);
    assert!(slot.is_some());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let observed_budget = {
        let prepared = prepare!(policy, receiver.context()).unwrap();
        observations::check(&prepared, &owner, 1000)
    };
    assert!(matches!(
        prepare!(
            Policy {
                maximum_storage_bytes: observed_budget - 1,
                ..policy
            },
            receiver.context()
        )
        .unwrap()
        .admit_observed(&mut owner, 1000, 1),
        Err(mixed_workset::Refusal::Bounds)
    ));
    assert_eq!(owner, before);
    assert!(slot.is_some());
    assert_eq!(effects.get(), 0);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let prepared = prepare!(policy, receiver.context()).unwrap();
    let observed_total = observations::check(&prepared, &owner, 1000);
    assert!(observed_total <= policy.maximum_storage_bytes);
    assert_eq!(owner, before);
    let (committed, cost) = crate::probe::observe(|| prepared.admit_observed(&mut owner, 1000, 1));
    let committed = committed.unwrap();
    assert_eq!(owner.admissions().len(), 2);
    assert_eq!(effects.get(), 0);
    assert!(slot.is_none());
    assert!(cost.peak_bytes + kernel_bytes <= observed_total);
    assert!(owner
        .admissions()
        .iter()
        .all(|a| &a.plan_id == context.plan_id()));
    assert_eq!(
        committed.approved.metadata().body_play,
        context.play_id().as_str()
    );
    assert_eq!(retained.payload(), Some(document.bytes()));
    let port = |direction| {
        let p = committed
            .approved
            .metadata()
            .fore_ports
            .chunks_exact(ASSIGNED_LOCAL_FORE_BYTES)
            .find(|p| p[4] == direction)
            .unwrap();
        (
            RemoteEndpointId(u16::from_le_bytes(p[..2].try_into().unwrap())),
            CordId(u16::from_le_bytes(p[2..4].try_into().unwrap())),
        )
    };
    let (ingress, incoming) = port(0);
    let (egress, outgoing) = port(1);
    let started = start::check(
        committed,
        &receiver,
        &retained,
        policy,
        &owner,
        &observations,
    );
    supervision::exercise(
        started,
        policy,
        receiver.context(),
        &retained,
        &mut owner,
        supervision::Flow {
            ingress: (ingress, incoming),
            egress: (egress, outgoing),
            input: values.0,
            expected: values.1,
        },
    );

    assert_eq!(effects.get(), 1);
    // Only explicit Root retirement released the original ledger scope.
    assert!(owner.admissions().is_empty());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    println!("complete signed mixed workset: {} bytes; {kernel_bytes} kernel/native backing plus Core bridge base {total}, with owned observations within {observed_total} requested-byte bound; missing/low evidence and one-byte shortage preserve both owners; original Body Plan/Play reservations survive typed CPU execution and kernel retirement; no physical Start",document.bytes().len());
}
