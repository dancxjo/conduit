//! Scripted native custody through ordinary Source, planning, Body and kernel.
//! These fixtures establish no physical attachment or hardware entropy proof.
use super::*;
use crate::{
    digital_base::{DigitalDisposition, DigitalRequest},
    monotonic_clock::codec::ClockDisposition,
    protocol_source::ProtocolSourcePackage,
    spi_base::{SpiDisposition, SpiTransaction},
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
mod owners;
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
    let counts = Rc::new(RefCell::new(Counts::default()));
    let provider = |index| Provider {
        counts: counts.clone(),
        index,
    };
    let (session, host, owners) = owners::prepare(
        &entry,
        [gears[0].clone(), gears[1].clone()],
        provider(0),
        [provider(1), provider(2)],
        provider(3),
    );
    Fixture {
        entry,
        session,
        host,
        owners,
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
