//! CPU transcript proof: one physical owner, exact two-client result routing.
extern crate std;
use super::*;
use crate::protocol_play::PreparedProtocolPlay;
use crate::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSpecializationRequest as Request,
    ProtocolValueReference,
};
use alloc::{sync::Arc, vec::Vec};
use std::sync::Mutex;
struct Bus {
    trace: Arc<Mutex<Vec<u8>>>,
}
impl I2cProvider for Bus {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        self.trace.lock().unwrap().push(request.address());
        if request.address() == 0x70 {
            return Err(I2cDisposition::NotAcknowledged);
        }
        assert_eq!(request.read_length(), 1);
        input[0] = request.address();
        Ok(1)
    }
    fn revoke(&mut self) {}
}
fn prepare(
    bus: Bus,
) -> (
    PreparedProtocolPlay<Bus>,
    StructuredInfoType,
    [ValuePayload; 2],
) {
    let reference = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let package = ProtocolSourcePackage::compile(
        include_str!("../../../../../../plots/device-protocols/i2c-two-clients.conduit").into(),
        &[
            Request::Merge {
                value: reference("I2cClientRequest"),
            },
            Request::ExactZip {
                left: reference("I2cClientRequest"),
                right: reference("I2cResult"),
            },
        ],
    )
    .unwrap();
    let source =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "i2c-two-clients")
            .unwrap();
    let input = source.input_schema(&port_id("first")).unwrap();
    let outputs = ["first_result", "second_result"].map(|name| ValuePayload {
        value_kind: source
            .output_schema(&port_id(name))
            .unwrap()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        encoded: Vec::with_capacity(4096),
    });
    let id = I2cNativeIdentity {
        base_implementation_id: "conduitos.base/i2c-controller@1".into(),
        host_id: "fixture/i2c-clients".into(),
        boot_id: "fixture/i2c-clients-boot".into(),
        base_id: "fixture/i2c".into(),
        provider_instance_id: "fixture/i2c-owner".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/i2c-pool".into(),
        resource_generation_id: ResourceGenerationId("fixture/i2c-generation".into()),
        envelope_id: "fixture/i2c-envelope".into(),
        artifact_id: "fixture/i2c-artifact".into(),
    };
    // SAFETY: inert, exclusively retained CPU bus. No hardware is accessible.
    let ready = unsafe {
        ReadyI2cBase::new(
            id.clone(),
            I2cAttachment {
                generation: 1,
                minimum_address: 0x70,
                maximum_address: 0x77,
                resource_bytes: 32,
            },
            bus,
        )
    }
    .unwrap();
    let mut host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: id.host_id.clone(),
        boot_id: id.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/protocol".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    ready
        .append_to_advertisement(&mut host, &I2cContract::prepare().unwrap())
        .unwrap();
    source.publish_pure_backs(&mut host).unwrap();
    let grants = [AuthorityGrant {
        grant_id: "fixture/i2c-grant".into(),
        contract_id: I2C_AUTHORITY.into(),
        host_call_contract_id: I2C_CALL.into(),
        subject_kind: "machine/i2c/transact".into(),
        host_id: id.host_id.clone(),
        boot_id: id.boot_id.clone(),
        capability_id: I2C_IMPLEMENTATION.into(),
    }];
    let hosts = [host];
    let placements = source.placements(&hosts).unwrap();
    let artifact = source
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
    let (definition, operations) = artifact.into_preparation_parts();
    let plan = &definition.internal_plan;
    let active = bind_active_play(&plan.plan_id, &id.host_id, &id.boot_id, 0);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.implementation_id.as_str() == I2C_IMPLEMENTATION)
        .unwrap();
    let (table, handle, claim) = super::common::possession(BaseCapabilityScope {
        host_id: id.host_id,
        boot_id: id.boot_id,
        base_instance_id: id.provider_instance_id,
        base_provider_generation: 1,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id,
        authority_grant_id: grants[0].grant_id.clone(),
        authority_contract_id: I2C_AUTHORITY.into(),
        capability_id: gear.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: I2C_CALL.into(),
        subject_kind: gear.kind_id.clone(),
        resource_pool_id: id.resource_pool_id,
        resource_generation_id: id.resource_generation_id,
        envelope_id: id.envelope_id,
        maximum_parameter_bytes: I2C_MAXIMUM_BYTES,
        maximum_result_bytes: I2C_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 8,
    });
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .filter(|p| p.implementation_id.as_str() == I2C_IMPLEMENTATION)
            .count(),
        1
    );
    (
        PreparedProtocolPlay::prepare_with_operations(
            definition, ready, table, handle, claim, operations,
        )
        .unwrap(),
        input,
        outputs,
    )
}
fn run_clients(requests: &[(&str, u8)], expected: [usize; 2]) {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let (mut play, input, mut outputs) = prepare(Bus {
        trace: trace.clone(),
    });
    assert!(trace.lock().unwrap().is_empty());
    play.start().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = input.shape() else {
        panic!("request")
    };
    for &(name, address) in requests {
        let value = StructuredInfoValue::record(
            input.clone(),
            fields
                .iter()
                .map(|field| {
                    let ty = field.value_type();
                    let value = match field.name() {
                        "address" => StructuredInfoValue::leaf(ty.clone(), vec![address]).unwrap(),
                        "read" => StructuredInfoValue::leaf(ty.clone(), vec![1]).unwrap(),
                        "write" => StructuredInfoValue::sequence(ty.clone(), vec![]).unwrap(),
                        _ => panic!("request field"),
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap();
        play.admit_input(
            &port_id(name),
            0,
            &ValuePayload {
                value_kind: input.profile().unwrap().value_kind().clone(),
                encoded: value.canonical_bytes().unwrap(),
            },
        )
        .unwrap();
    }
    for name in ["first", "second"] {
        play.close_input(&port_id(name)).unwrap();
    }
    let mut seen = [0; 2];
    let mut status = conduit_composite::KernelCompositeStatus::Active;
    for _ in 0..512 {
        status = play.step().unwrap();
        for (index, (name, output)) in ["first_result", "second_result"]
            .into_iter()
            .zip(&mut outputs)
            .enumerate()
        {
            if let Some(sequence) = play.output_into(&port_id(name), output).unwrap() {
                seen[index] += 1;
                let result = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
                let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
                    panic!("result")
                };
                assert_eq!(
                    tag,
                    if index == 0 {
                        "completed"
                    } else {
                        "not-acknowledged"
                    }
                );
                if index == 0 {
                    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
                        panic!("read result")
                    };
                    let StructuredInfoValueShape::Collection(values) = fields[0].value().shape()
                    else {
                        panic!("read bytes")
                    };
                    assert_eq!(values.len(), 1);
                    assert!(
                        matches!(values[0].shape(),StructuredInfoValueShape::Leaf(bytes) if bytes==[0x76])
                    );
                }
                play.complete_output(&port_id(name), sequence).unwrap();
            }
        }
        if status == conduit_composite::KernelCompositeStatus::Complete {
            break;
        }
    }
    assert_eq!(
        status,
        conduit_composite::KernelCompositeStatus::Complete,
        "seen={seen:?}, bus={:?}",
        trace.lock().unwrap()
    );
    assert_eq!(seen, expected);
    let trace = trace.lock().unwrap();
    assert_eq!(trace.len(), requests.len());
    let mut actual = trace.clone();
    let mut expected: Vec<_> = requests.iter().map(|(_, address)| *address).collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn two_clients_one_owner_preserves_reply_identity_and_failure() {
    run_clients(&[("first", 0x76), ("second", 0x70)], [1, 1]);
    run_clients(&[("second", 0x70), ("first", 0x76)], [1, 1]);
}
#[test]
fn closing_one_client_does_not_close_the_other() {
    run_clients(&[("first", 0x76)], [1, 0]);
    run_clients(&[("second", 0x70)], [0, 1]);
}
