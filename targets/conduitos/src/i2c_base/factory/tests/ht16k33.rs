//! CPU kernel conformance with inert bus possession; no physical/Body claim.
extern crate std;
use super::*;
use crate::protocol_play::PreparedProtocolPlay;
use crate::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSpecializationRequest as Request,
    ProtocolValueReference,
};
use alloc::{string::String, sync::Arc, vec::Vec};
use std::sync::Mutex;

struct Bus {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    fail_at: Option<usize>,
}
impl I2cProvider for Bus {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert_eq!(request.address(), 0x70);
        assert!(input.is_empty());
        assert_eq!(request.read_length(), 0);
        let mut writes = self.writes.lock().unwrap();
        let index = writes.len();
        writes.push(request.write().to_vec());
        if self.fail_at == Some(index) {
            Err(I2cDisposition::NotAcknowledged)
        } else {
            Ok(0)
        }
    }
    fn revoke(&mut self) {}
}
fn package() -> ProtocolSourcePackage {
    let units = [
        include_str!("../../../../../../plots/device-protocols/ht16k33.conduit"),
        include_str!("../../../../../../plots/device-protocols/ht16k33-exchanges.conduit"),
        include_str!("../../../../../../plots/device-protocols/ht16k33-feedback.conduit"),
    ];
    let imports = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| l.starts_with("with "))
        .collect::<alloc::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let bodies = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| !l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let reference = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        alloc::format!("{imports}\n{bodies}"),
        &[
            Request::SeededUntil {
                value: reference("Ht16k33ProtocolFrame"),
            },
            Request::FeedbackZip {
                left: reference("Ht16k33State"),
                right: reference("I2cResult"),
            },
            Request::Merge {
                value: reference("Ht16k33Failure"),
            },
        ],
    )
    .unwrap()
}
fn prepare(
    bus: Bus,
    initialization: bool,
) -> (
    PreparedProtocolPlay<Bus>,
    StructuredInfoType,
    [ValuePayload; 2],
) {
    let package = package();
    let source = PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package).unwrap(),
        if initialization {
            "ht16k33-initialize-display"
        } else {
            "ht16k33-write-display"
        },
    )
    .unwrap();
    let input = source
        .input_schema(&port_id(if initialization {
            "configuration"
        } else {
            "frame"
        }))
        .unwrap();
    let outputs = ["acknowledged", "refused"].map(|name| ValuePayload {
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
        host_id: "fixture/ht16k33".into(),
        boot_id: "fixture/ht16k33-boot".into(),
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
        maximum_operations: if initialization { 5 } else { 1 },
    });
    (
        PreparedProtocolPlay::prepare_with_operations(
            definition, ready, table, handle, claim, operations,
        )
        .unwrap(),
        input,
        outputs,
    )
}
fn value(ty: &StructuredInfoType, address: u8, initialization: bool) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("input")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let ty = field.value_type();
                let value = if field.name() == "ram" {
                    assert!(!initialization);
                    let StructuredInfoTypeShape::Collection { element, .. } = ty.shape() else {
                        panic!("RAM")
                    };
                    StructuredInfoValue::collection(
                        ty.clone(),
                        (0..16)
                            .map(|n| StructuredInfoValue::leaf(element.clone(), vec![n]).unwrap())
                            .collect(),
                    )
                    .unwrap()
                } else {
                    StructuredInfoValue::leaf(
                        ty.clone(),
                        vec![match field.name() {
                            "address" => address,
                            "brightness" => 3,
                            "blink" => 0,
                            "enabled" => 1,
                            _ => panic!("field"),
                        }],
                    )
                    .unwrap()
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn run(
    initialization: bool,
    address: u8,
    fail_at: Option<usize>,
) -> (Vec<Vec<u8>>, String, ValuePayload) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let (mut play, ty, mut outputs) = prepare(
        Bus {
            writes: writes.clone(),
            fail_at,
        },
        initialization,
    );
    assert!(
        writes.lock().unwrap().is_empty(),
        "preparation has no bus effects"
    );
    let input = value(&ty, address, initialization);
    play.start().unwrap();
    let port = port_id(if initialization {
        "configuration"
    } else {
        "frame"
    });
    play.admit_input(
        &port,
        0,
        &ValuePayload {
            value_kind: ty.profile().unwrap().value_kind().clone(),
            encoded: input.canonical_bytes().unwrap(),
        },
    )
    .unwrap();
    play.close_input(&port).unwrap();
    let mut observed = None;
    let mut status = conduit_composite::KernelCompositeStatus::Active;
    for _ in 0..512 {
        status = play.step().unwrap();
        for (name, output) in ["acknowledged", "refused"].into_iter().zip(&mut outputs) {
            if let Some(sequence) = play.output_into(&port_id(name), output).unwrap() {
                assert!(observed.is_none(), "one terminal result");
                if name == "acknowledged" {
                    let receipt =
                        StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
                    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
                        panic!("acknowledgement record")
                    };
                    assert_eq!(fields.len(), 1);
                    assert_eq!(fields[0].name(), "transactions");
                    assert!(
                        matches!(fields[0].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [if initialization { 5 } else { 1 }])
                    );
                }
                observed = Some((String::from(name), output.clone()));
                play.complete_output(&port_id(name), sequence).unwrap();
            }
        }
        if status == conduit_composite::KernelCompositeStatus::Complete {
            break;
        }
    }
    assert_eq!(status, conduit_composite::KernelCompositeStatus::Complete);
    let trace = writes.lock().unwrap().clone();
    let (terminal, output) = observed.expect("terminal result");
    (trace, terminal, output)
}
#[test]
fn ht16k33_initialization_feedback_runs_in_the_ordinary_kernel() {
    let (writes, terminal, _) = run(true, 0x70, None);
    assert_eq!(terminal, "acknowledged");
    assert_eq!(
        writes,
        [vec![0x21], vec![0x80], vec![0; 17], vec![0xe3], vec![0x81]]
    );
}
#[test]
fn ht16k33_feedback_failure_stops_before_enabling_without_retry() {
    let (writes, terminal, output) = run(true, 0x70, Some(2));
    assert_eq!(terminal, "refused");
    assert_eq!(writes, [vec![0x21], vec![0x80], vec![0; 17]]);
    let failure = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = failure.shape() else {
        panic!("failure")
    };
    assert_eq!(tag, "bus");
    assert!(
        matches!(payload.shape(), StructuredInfoValueShape::Variant { tag, .. } if tag == "not-acknowledged")
    );
}
#[test]
fn ht16k33_redraw_runs_as_one_planned_transaction() {
    let (writes, terminal, _) = run(false, 0x70, None);
    assert_eq!(terminal, "acknowledged");
    assert_eq!(
        writes,
        [vec![
            0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
        ]]
    );
}
#[test]
fn ht16k33_redraw_bus_failure_has_one_terminal_refusal() {
    let (writes, terminal, _) = run(false, 0x70, Some(0));
    assert_eq!(terminal, "refused");
    assert_eq!(writes.len(), 1);
}
#[test]
fn ht16k33_unsupported_protocol_address_has_no_bus_effect() {
    for initialization in [false, true] {
        let (writes, terminal, _) = run(initialization, 0x6f, None);
        assert_eq!(terminal, "refused");
        assert!(writes.is_empty());
    }
}

#[test]
fn ht16k33_exchange_fores_do_not_claim_a_controller() {
    let bytes = serde_json::to_vec(&package()).unwrap();
    let contract = I2cContract::prepare().unwrap();
    for entry in ["ht16k33-initialize-exchange", "ht16k33-write-exchange"] {
        let protocol = PreparedProtocolEntry::prepare(&bytes, entry).unwrap();
        assert!(protocol
            .expanded()
            .expanded
            .gears
            .iter()
            .all(|gear| gear.kind_id.as_str() != "machine/i2c/transact"));
        assert_eq!(
            protocol.input_schema(&port_id("result")).unwrap(),
            contract.decode_result_type().unwrap()
        );
        assert_eq!(
            protocol.output_schema(&port_id("request")).unwrap(),
            contract.decode_request_type().unwrap()
        );
    }
}
