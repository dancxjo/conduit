//! The native two-slot ledger must admit queued + newly completed observations.
use super::*;
use conduit_kernel::{HostedValueStore, ValueStorage};
fn definition(resident: bool) -> NativeSourceDefinition {
    use conduit_std_offers::*;
    let kind = if resident {
        conduit_semantic_catalog::ambient_resident_observe_kind()
    } else {
        conduit_semantic_catalog::ambient_observe_kind()
    }
    .unwrap();
    let offer = if resident {
        ambient_resident_source_offer()
    } else {
        ambient_source_offer()
    }
    .unwrap();
    let grant = AuthorityGrant {
        grant_id: "fixture/native-budget".into(),
        contract_id: if resident {
            AMBIENT_RESIDENT_SOURCE_AUTHORITY
        } else {
            AMBIENT_SOURCE_AUTHORITY
        }
        .into(),
        host_call_contract_id: offer.host_calls[0].contract_id.clone(),
        subject_kind: kind.kind_id.clone(),
        host_id: "host/budget".into(),
        boot_id: "boot/budget".into(),
        capability_id: offer.capability_id.clone(),
    };
    NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: "base/ambient".into(),
            provider_instance_id: "fixture/ambient".into(),
            provider_generation: 1,
            implementation_id: offer.implementation.implementation_id.clone(),
            mechanism_family: if resident {
                AMBIENT_RESIDENT_SOURCE_BASE_KIND
            } else {
                AMBIENT_SOURCE_BASE_KIND
            }
            .into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![],
        },
        resource_generation: ResourceGenerationId("fixture/generation".into()),
        configuration: vec![],
        input_bytes: 8,
        output_bytes: 8192,
        resource_bytes: 8192,
    }
}
#[test]
fn exact_ambient_two_slots_hold_queued_and_pending_full_outputs_without_growth() {
    for resident in [false, true] {
        let d = definition(resident);
        assert_eq!(value_byte_budget(&d), 16384);
        let mut values =
            HostedValueStore::new_with_slot_capacities(&[8192, 8192], value_byte_budget(&d))
                .unwrap();
        let backing = values.reserved_storage_bytes();
        let bytes = [0u8; 8192];
        let probe = crate::allocation_probe::begin();
        let queued = values.store(&bytes).unwrap();
        values.retain(queued).unwrap();
        let pending = values.store(&bytes).unwrap();
        assert_eq!(values.reference_count(queued).unwrap(), 2);
        assert!(
            values.store(&[0]).is_err(),
            "the existing two slots remain finite"
        );
        values.release(pending).unwrap();
        let replacement = values.store(&bytes).unwrap();
        values.release(replacement).unwrap();
        values.release(queued).unwrap();
        values.release(queued).unwrap();
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
        assert_eq!(values.reserved_storage_bytes(), backing);
    }
}
#[test]
fn compiler_and_other_native_profiles_keep_original_input_plus_output_ledger() {
    let mut d = definition(true);
    d.kind.kind_id = conduit_std_offers::SOURCE_CHECK_KIND.into();
    d.base.capabilities[0].implementation.implementation_id =
        conduit_std_offers::SOURCE_CHECK_IMPLEMENTATION.into();
    d.input_bytes = 16384;
    d.output_bytes = 65536;
    assert_eq!(value_byte_budget(&d), 81920);
    d.input_bytes = 8;
    d.output_bytes = 8192;
    assert_eq!(
        value_byte_budget(&d),
        8200,
        "matching sizes alone do not widen another native profile"
    );
}
