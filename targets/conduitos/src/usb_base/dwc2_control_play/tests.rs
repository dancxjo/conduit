//! Explicit virtual-bus authority joins normal kernel calls to real lease code.
use super::*;
use crate::arch::dwc2::test_io;
fn native() -> (UsbNativeIdentity, NativeProtocolIssuer) {
    let identity = UsbNativeIdentity {
        host_id: "fixture/control-play-host".into(),
        boot_id: "fixture/control-play-boot".into(),
        base_id: "fixture/controller".into(),
        provider_instance_id: "fixture/owned-bus".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pio".into(),
        resource_generation_id: ResourceGenerationId("fixture/pio-generation".into()),
        envelope_id: "fixture/bus-window".into(),
        artifact_id: "fixture/compiled-control".into(),
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: "fixture/bus-authority".into(),
            contract_id: super::super::control_factory::CONTROL_AUTHORITY.into(),
            host_call_contract_id: CONTROL_CALL.into(),
            subject_kind: CONTROL_KIND.into(),
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            capability_id: super::super::control_factory::CONTROL_IMPLEMENTATION.into(),
        },
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: identity.provider_generation,
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        operation_contract_id: CONTROL_CALL.into(),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 4,
    };
    let issuer = NativeProtocolIssuer::from_test_authority(authority, [8; 32]).unwrap();
    (identity, issuer)
}
fn completion(bytes: Option<&[u8]>) -> (u32, Vec<u32>, Vec<u32>) {
    let Some(bytes) = bytes else {
        return (3, vec![], vec![]);
    };
    let words = bytes
        .chunks(4)
        .map(|b| {
            let mut a = [0; 4];
            a[..b.len()].copy_from_slice(b);
            u32::from_le_bytes(a)
        })
        .collect();
    (3, vec![(2 << 17) | ((bytes.len() as u32) << 4)], words)
}
#[test]
fn dwc2_control_play_preserves_bus_custody_and_reuses_admitted_calls_without_allocations() {
    let (identity, issuer) = native();
    let mut controller = test_io::prepared();
    for _ in 0..2 {
        test_io::registers(&mut controller).script.extend([
            completion(None),
            completion(Some(&[9; 8])),
            completion(None),
        ]);
    }
    test_io::registers(&mut controller).writes.reserve(512);
    let route = RootControlRoute {
        slot: 1,
        generation: 1,
        address: 0,
        maximum_packet: 8,
        translator: None,
    };
    let reservation = unsafe { BusControlReservation::admit_owned_bus(1, 2, 7) }.unwrap();
    let mut run =
        unsafe { Dwc2ControlPlay::prepare_bus(controller, route, reservation, &identity, issuer) }
            .unwrap();
    let contract = ControlContract::prepare().unwrap();
    let ty = contract.request_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    let request = StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                let value = if f.name() == "setup" {
                    StructuredInfoValue::leaf(
                        f.value_type().clone(),
                        0x0008000001000680_u64.to_le_bytes().to_vec(),
                    )
                    .unwrap()
                } else {
                    StructuredInfoValue::sequence(f.value_type().clone(), vec![]).unwrap()
                };
                StructuredFieldValue::new(f.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let allocations = crate::test_allocations::allocations(|| {
        run.start().unwrap();
        for address in [0, 3] {
            run.submit(RootControlRoute { address, ..route }, &request)
                .unwrap();
            for _ in 0..64 {
                run.step().unwrap();
                if run.result().is_some() {
                    break;
                }
            }
            let result =
                validate_canonical_structured_value(&run.result().unwrap().encoded).unwrap();
            assert!(result.variant_payload("completed").unwrap().is_some());
            run.acknowledge_result();
        }
    });
    assert_eq!(allocations, 0);
    let controller = run.retire().map_err(|(_, e)| e).unwrap();
    assert!(controller.is_quiescent());
}
