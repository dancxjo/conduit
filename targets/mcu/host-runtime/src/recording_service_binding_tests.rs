//! A retained original-owner descriptor is never evidence of live admission.
use super::*;
use crate::member_return_tests::return_allocation_probe as allocation;
#[test]
fn real_residence_binding_is_bounded_exact_and_preserves_recovered_and_ready_owners() {
    with_context(|context, policy| {
        let storage = Storage::new();
        let reads = storage.reads.clone();
        let programs = storage.programs.clone();
        let drops = storage.drops.clone();
        let mut equipment = selected(storage, context.actual, &context.bootstrap.body_id);
        let scanned = reads.get();
        let required =
            "service/pool".len() + conduit_resource_snapshot::native_append::APPEND_CLASS.len();
        let (refused, cost) = allocation::observe(|| equipment.resource_binding(required - 1));
        assert!(
            matches!(refused, Err(Refusal::Capacity { required: actual, maximum }) if actual == required && maximum == required - 1)
        );
        assert_eq!(cost.allocation_attempts, 0);
        assert!(matches!(equipment.stage, Some(Stage::Recovered { .. })));
        let binding = equipment.resource_binding(required).unwrap().unwrap();
        assert_eq!(binding.pool_id.as_str(), "service/pool");
        assert_eq!(
            binding.class_id.as_str(),
            conduit_resource_snapshot::native_append::APPEND_CLASS
        );
        assert_eq!(binding.units, 1);
        assert!(
            binding.protected.is_none() && binding.compute.is_none() && binding.content.is_none()
        );
        assert!(binding.pool_id.0.capacity() + binding.class_id.0.capacity() <= required);
        for failed_allocation in [1, 2] {
            let (refused, cost) = allocation::observe_refusing(failed_allocation, || {
                equipment.resource_binding(required)
            });
            assert!(matches!(refused, Err(Refusal::Allocation)));
            assert_eq!(cost.refused_allocations, 1);
            assert!(matches!(equipment.stage, Some(Stage::Recovered { .. })));
        }
        let [append, read] = operations();
        let mut append_issuer = Issuer::new();
        let mut read_issuer = Issuer::new();
        equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096,
            )
            .unwrap();
        let address = equipment.ready().unwrap() as *const _;
        assert_eq!(equipment.resource_binding(required).unwrap(), Some(binding));
        assert!(matches!(
            equipment.resource_binding(required - 1),
            Err(Refusal::Capacity { .. })
        ));
        assert_eq!(equipment.ready().unwrap() as *const _, address);
        assert!(equipment.ready_slot().unwrap().is_some());
        assert_eq!(
            equipment.ready().unwrap().original_grants()[0],
            *append_issuer.grant().unwrap()
        );
        assert_eq!(
            equipment.ready().unwrap().original_grants()[1],
            *read_issuer.grant().unwrap()
        );
        assert_eq!(reads.get(), scanned);
        assert_eq!(programs.get(), 0);
        assert_eq!(drops.get(), 0);
    });
}
#[test]
fn absent_busy_and_unrecovered_owners_never_create_binding_descriptions() {
    with_context(|context, policy| {
        let storage = Storage::new();
        let reads = storage.reads.clone();
        // SAFETY: sole CPU media, no service or execution selected.
        let unselected = unsafe {
            Equipment::new(
                storage,
                identity(context.actual),
                &context.bootstrap.body_id,
                &[],
                RESERVED_PREFIX_BYTES,
                16,
            )
        };
        let (none, cost) = allocation::observe(|| unselected.resource_binding(0));
        assert_eq!(none.unwrap(), None);
        assert_eq!(cost.allocation_attempts, 0);
        assert!(unselected.unused_storage().is_some());
        assert_eq!(reads.get(), 0);
        let storage = Storage::new();
        let bytes = storage.bytes.clone();
        let drops = storage.drops.clone();
        let programs = storage.programs.clone();
        let reads = storage.reads.clone();
        let mut equipment = selected(storage, context.actual, &context.bootstrap.body_id);
        // Actual occupied media appearing after the initial scan makes append
        // refuse with RecoveryRequired, retaining that same unrecovered provider.
        bytes.borrow_mut()[RESERVED_PREFIX_BYTES as usize] = 0;
        let Some(Stage::Recovered { provider, .. }) = equipment.stage.as_mut() else {
            panic!("recovered owner")
        };
        assert!(matches!(
            provider.append(b"must not overwrite"),
            Err(RecordResidenceRefusal::RecoveryRequired)
        ));
        let before = reads.get();
        let (refused, cost) = allocation::observe(|| equipment.resource_binding(4096));
        assert!(refused.is_err());
        assert_eq!(cost.allocation_attempts, 0);
        assert_eq!(reads.get(), before);
        assert_eq!(programs.get(), 0);
        assert_eq!(drops.get(), 0);
        let Some(Stage::Recovered { provider, .. }) = equipment.stage.as_mut() else {
            panic!("same provider retained")
        };
        provider.recover().unwrap();
        let [append, read] = operations();
        let mut append_issuer = Issuer::new();
        let mut read_issuer = Issuer::new();
        equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096,
            )
            .unwrap();
        // Simulate the original Start's sole Ready being held by its caller;
        // no actual operation or media write is performed by this fixture.
        let original = equipment.ready_slot().unwrap().take().unwrap();
        let (refused, cost) = allocation::observe(|| equipment.resource_binding(4096));
        assert!(matches!(refused, Err(Refusal::Scope)));
        assert_eq!(cost.allocation_attempts, 0);
        assert_eq!(drops.get(), 0);
        *equipment.ready_slot().unwrap() = Some(original);
        assert!(equipment.resource_binding(4096).unwrap().is_some());
    });
}
