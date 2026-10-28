//! Scripted native custody through ordinary Source, planning, Body and kernel.
//! These fixtures establish no physical attachment or hardware entropy proof.
use super::*;
use crate::{
    digital_base::{
        DigitalDisposition, DigitalRequest,
        installation::{DIGITAL_AUTHORITY, DigitalNativeIdentity},
        owner::DigitalAttachment,
    },
    monotonic_clock::{
        codec::ClockDisposition,
        installation::{CLOCK_AUTHORITY, ClockNativeIdentity},
        owner::MAXIMUM_POLL_STEPS,
    },
    protocol_source::ProtocolSourcePackage,
    spi_base::{
        SpiDisposition, SpiTransaction,
        installation::{SPI_AUTHORITY, SpiNativeIdentity},
        owner::SpiAttachment,
    },
};
use alloc::{format, rc::Rc};
use core::cell::RefCell;

const SOURCE: &str = r#"
with machine/spi/exchange/request as SpiRequest
with machine/spi/exchange/result as SpiResult
with machine/digital/access/request as DigitalRequest
with machine/digital/access/result as DigitalResult
with machine/clock/at/request as ClockRequest
with machine/clock/at/result as ClockResult
plot peripherals (
 >> spi-in: SpiRequest...|
 >> a-in: DigitalRequest...|
 >> b-in: DigitalRequest...|
 >> clock-in: ClockRequest...|
 spi-out: SpiResult...| >>
 a-out: DigitalResult...| >>
 b-out: DigitalResult...| >>
 clock-out: ClockResult...| >>
) {
 spi: machine/spi/exchange
 a: machine/digital/access
 b: machine/digital/access
 clock: machine/clock/at
 spi-in >> spi >> spi-out
 a-in >> a >> a-out
 b-in >> b >> b-out
 clock-in >> clock >> clock-out
}
"#;
#[derive(Default)]
struct Counts {
    effects: [u32; 4],
    revoked: [bool; 4],
}
struct Provider {
    counts: Rc<RefCell<Counts>>,
    index: usize,
}
impl Provider {
    fn effect(&self) {
        let mut counts = self.counts.borrow_mut();
        assert!(!counts.revoked[self.index]);
        counts.effects[self.index] += 1;
    }
    fn stop(&self) {
        self.counts.borrow_mut().revoked[self.index] = true;
    }
}
impl SpiProvider for Provider {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        assert_eq!(request.output(), [0x11, 0x22, 0x33]);
        self.effect();
        input.copy_from_slice(&[0xaa, 0xbb, 0xcc]);
        Ok(())
    }
    fn revoke(&mut self) {
        self.stop();
    }
}
impl DigitalProvider for Provider {
    fn access(&mut self, request: DigitalRequest) -> Result<bool, DigitalDisposition> {
        assert_eq!(request, DigitalRequest::Observe);
        self.effect();
        Ok(self.index == 1)
    }
    fn revoke(&mut self) {
        self.stop();
    }
}
impl MonotonicDeadlineProvider for Provider {
    fn poll_until(&mut self, deadline: u64) -> Result<Option<u64>, ClockDisposition> {
        assert_eq!(deadline, 10);
        self.effect();
        Ok(Some(10))
    }
    fn revoke(&mut self) {
        self.stop();
    }
}
fn authority(
    name: &str,
    contract: &str,
    call: &str,
    kind: &str,
    capability: &str,
    bytes: u32,
    work: u64,
) -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: format!("grant/{name}").into(),
            contract_id: contract.into(),
            host_call_contract_id: call.into(),
            subject_kind: kind.into(),
            host_id: "host/peripheral-fixture".into(),
            boot_id: "boot/peripheral-fixture".into(),
            capability_id: capability.into(),
        },
        base_instance_id: format!("provider/{name}").into(),
        base_provider_generation: 1,
        resource_pool_id: format!("resource/{name}").into(),
        resource_generation_id: ResourceGenerationId(format!("generation/{name}/1")),
        operation_contract_id: call.into(),
        envelope_id: format!("envelope/{name}").into(),
        maximum_parameter_bytes: bytes,
        maximum_result_bytes: bytes,
        maximum_work_units: work,
        maximum_in_flight: 1,
        maximum_operations: 8,
    }
}
fn issuer(a: BaseCapabilityAuthority, key: u8) -> NativeProtocolIssuer {
    NativeProtocolIssuer::from_test_authority(a, [key; 32]).unwrap()
}
struct Fixture {
    entry: PreparedProtocolEntry,
    session: BodyLifecycleSession,
    host: HostAdvertisement,
    owners: NativePeripheralOwners<Provider>,
    counts: Rc<RefCell<Counts>>,
    gears: Vec<GearId>,
}
fn fixture() -> Fixture {
    let package = ProtocolSourcePackage::compile(SOURCE.into(), &[]).unwrap();
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "peripherals")
            .unwrap();
    let gears: Vec<_> = entry
        .expanded()
        .expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == "machine/digital/access")
        .map(|gear| gear.gear_id.clone())
        .collect();
    assert_eq!(gears.len(), 2);
    let spi_a = authority(
        "spi",
        SPI_AUTHORITY,
        crate::spi_base::contract::SPI_CALL,
        "machine/spi/exchange",
        "conduitos/spi-exchange@1",
        4096,
        1,
    );
    let clock_a = authority(
        "clock",
        CLOCK_AUTHORITY,
        crate::monotonic_clock::contract::CLOCK_CALL,
        "machine/clock/at",
        "conduitos/monotonic-clock-at@1",
        512,
        MAXIMUM_POLL_STEPS,
    );
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: spi_a.grant.host_id.clone(),
        boot_id: spi_a.grant.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let expanded = &entry.expanded().expanded;
    let session = crate::protocol_test_support::protocol_body_for_resident(
        &conduit_body::ResidentPlot::new(
            expanded.source_document_id.clone(),
            expanded.checked_plot_id.clone(),
        ),
        &host.host_id,
        &host.boot_id,
    );
    let counts = Rc::new(RefCell::new(Counts::default()));
    let provider = |index| Provider {
        counts: counts.clone(),
        index,
    };
    let spi = unsafe {
        ReadySpiBase::new(
            SpiNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "base/spi".into(),
                provider_instance_id: spi_a.base_instance_id.clone(),
                provider_generation: 1,
                resource_pool_id: spi_a.resource_pool_id.clone(),
                resource_generation_id: spi_a.resource_generation_id.clone(),
                envelope_id: spi_a.envelope_id.clone(),
                artifact_id: "artifact/fixture".into(),
            },
            SpiAttachment {
                generation: 1,
                resource_bytes: 512,
            },
            provider(0),
        )
    }
    .unwrap()
    .erase();
    let mut digital = Vec::new();
    for (index, gear) in gears.iter().enumerate() {
        let name = format!("contact-{index}");
        let capability = format!("capability/{name}");
        let a = authority(
            &name,
            DIGITAL_AUTHORITY,
            crate::digital_base::contract::DIGITAL_CALL,
            "machine/digital/access",
            &capability,
            512,
            1,
        );
        let ready = unsafe {
            ReadyDigitalBase::new(
                DigitalNativeIdentity {
                    capability_id: capability.into(),
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    base_id: format!("base/{name}").into(),
                    provider_instance_id: a.base_instance_id.clone(),
                    provider_generation: 1,
                    resource_pool_id: a.resource_pool_id.clone(),
                    resource_generation_id: a.resource_generation_id.clone(),
                    envelope_id: a.envelope_id.clone(),
                    artifact_id: "artifact/fixture".into(),
                },
                DigitalAttachment {
                    generation: 1,
                    resource_bytes: 1,
                },
                provider(index + 1),
            )
        }
        .unwrap()
        .erase();
        digital.push(NativeDigitalOwner {
            gear: gear.clone(),
            ready,
            issuer: issuer(a, index as u8 + 2),
        });
    }
    let clock = unsafe {
        ReadyClockBase::new(
            ClockNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "base/clock".into(),
                provider_instance_id: clock_a.base_instance_id.clone(),
                provider_generation: 1,
                resource_pool_id: clock_a.resource_pool_id.clone(),
                resource_generation_id: clock_a.resource_generation_id.clone(),
                envelope_id: clock_a.envelope_id.clone(),
                artifact_id: "artifact/fixture".into(),
            },
            provider(3),
        )
    }
    .unwrap();
    Fixture {
        entry,
        session,
        host,
        owners: NativePeripheralOwners {
            spi,
            spi_issuer: issuer(spi_a, 1),
            digital,
            clock,
            clock_issuer: issuer(clock_a, 4),
        },
        counts,
        gears,
    }
}
fn record(schema: &StructuredInfoType, bytes: &[u8]) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("record");
    };
    StructuredInfoValue::record(
        schema.clone(),
        vec![
            StructuredFieldValue::new(
                fields[0].name(),
                StructuredInfoValue::leaf(fields[0].value_type().clone(), bytes.to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn observe(schema: &StructuredInfoType) -> Vec<u8> {
    let StructuredInfoTypeShape::Variant { cases, .. } = schema.shape() else {
        panic!("variant");
    };
    let payload = cases
        .iter()
        .find(|case| case.tag() == "observe")
        .unwrap()
        .payload_type();
    StructuredInfoValue::variant(
        schema.clone(),
        "observe",
        StructuredInfoValue::leaf(payload.clone(), vec![]).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn exact_native_contacts_prepare_without_effects_and_all_four_fors_dispatch_in_one_body() {
    let f = fixture();
    let mut play = prepare_native_peripheral(f.entry, f.session, f.host, f.owners, 1).unwrap();
    assert_eq!(f.counts.borrow().effects, [0; 4]);
    assert_eq!(f.counts.borrow().revoked, [false; 4]);
    let definition = play.kernel().unwrap().definition();
    for (index, gear) in f.gears.iter().enumerate() {
        let placement = definition.internal_plan.fragments[0]
            .placements
            .iter()
            .find(|p| &p.gear_id == gear)
            .unwrap();
        assert_eq!(
            placement.capability_id.as_str(),
            format!("capability/contact-{index}")
        );
        assert_eq!(
            placement
                .base
                .as_ref()
                .unwrap()
                .provider_instance_id
                .as_str(),
            format!("provider/contact-{index}")
        );
    }
    let inputs = definition.external_capability.inputs.clone();
    let outputs = definition.external_capability.outputs.clone();
    let spi = SpiContract::prepare().unwrap();
    let digital = DigitalContract::prepare().unwrap();
    let clock = MonotonicClockContract::prepare().unwrap();
    play.start().unwrap();
    assert!(play.session().realization().unwrap().play.is_some());
    for input in inputs {
        let encoded = match input.port_id.as_str() {
            "spi-in" => record(spi.request_type(), &[0x11, 0x22, 0x33]),
            "a-in" | "b-in" => observe(digital.request_type()),
            "clock-in" => record(clock.request_type(), &10u64.to_le_bytes()),
            _ => panic!("unknown Fore"),
        };
        play.admit_input(
            &input.port_id,
            0,
            &ValuePayload {
                value_kind: input.value_kind,
                encoded,
            },
        )
        .unwrap();
        play.close_input(&input.port_id).unwrap();
    }
    let mut observed = alloc::collections::BTreeSet::new();
    for _ in 0..96 {
        play.step().unwrap();
        for output in &outputs {
            if observed.contains(&output.port_id) {
                continue;
            }
            let mut payload = ValuePayload {
                value_kind: output.value_kind.clone(),
                encoded: Vec::with_capacity(4096),
            };
            if let Some(sequence) = play.output_into(&output.port_id, &mut payload).unwrap() {
                assert_eq!(sequence, 0);
                let result = validate_canonical_structured_value(&payload.encoded).unwrap();
                assert_eq!(result.variant_tag().unwrap(), "completed");
                play.complete_output(&output.port_id, sequence).unwrap();
                observed.insert(output.port_id.clone());
            }
        }
        if observed.len() == 4 {
            break;
        }
    }
    assert_eq!(observed.len(), 4);
    assert_eq!(f.counts.borrow().effects, [1; 4]);
    play.cancel().unwrap();
    assert_eq!(f.counts.borrow().revoked, [true; 4]);
    assert!(
        play.session()
            .realization()
            .is_none_or(|current| current.play.is_none())
    );
}
#[test]
fn wrong_and_duplicate_gear_selection_drop_every_owned_contact_without_effects() {
    for duplicate in [false, true] {
        let mut f = fixture();
        f.owners.digital[1].gear = if duplicate {
            f.gears[0].clone()
        } else {
            "gear/absent".into()
        };
        let refusal = prepare_native_peripheral(f.entry, f.session, f.host, f.owners, 1)
            .err()
            .expect("wrong selection refuses");
        assert!(matches!(refusal.reason, NativePeripheralReason::Selection));
        assert_eq!(f.counts.borrow().effects, [0; 4]);
        assert_eq!(&f.counts.borrow().revoked[..3], &[true; 3]);
        assert!(refusal.session.realization().is_none());
        // Generic clock Drop revocation is deliberately not claimed by this test.
    }
}
