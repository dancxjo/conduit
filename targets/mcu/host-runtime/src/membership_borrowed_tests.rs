//! Arrival header representation proof, not allocation proof for the exchange.
use super::BorrowedArrival;
use conduit_body::{PicoAdmissionAdvertisement, PICO_ADMISSION_PROTOCOL};
use conduit_core::{
    capability_offer_from_parts, resource_offer, CapabilityLimits, ImplementationOffer,
    PortDescriptor, PortDirection, PortTemporal,
};
use std::vec;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn borrowed_arrival_matches_owned_header_bytes_without_serialization_allocations() {
    let (_, mut actual, _, _) = crate::membership_tests::fixture();
    actual
        .resources
        .push(resource_offer("fixture/pool", "fixture/memory", 4096));
    // A nonempty nested capability exercises the metadata that must remain in
    // its original owner rather than being cloned for an arrival header.
    actual.capabilities.push(capability_offer_from_parts! {
        semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
        capability_id: "fixture/observation".into(), kind_id: "fixture/observation".into(),
        kind_contract_revision: "fixture/observation@1".into(),
        implementation: ImplementationOffer {
            execution_profile_id: "fixture/bounded@1".into(),
            implementation_id: "fixture/native@1".into(), artifact_id: "fixture/image@1".into(),
        },
        inputs: vec![], outputs: vec![PortDescriptor {
            port_id: "reading".into(), value_kind: "value/count".into(),
            direction: PortDirection::Output, temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
        limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 64 },
    });
    let key = core::array::from_fn::<_, 32, _>(|index| (index * 8 + 7) as u8);
    for (label, freshness) in [
        ("", 1),
        ("Green", 100),
        ("room \"north\"\\shelf\n\t", u64::MAX),
        ("Hamm Creek · serre 🌱", 7),
    ] {
        let owned = PicoAdmissionAdvertisement {
            protocol: PICO_ADMISSION_PROTOCOL,
            advertisement: actual.clone(),
            friendly_label: label.into(),
            verifying_key: key.to_vec(),
            freshness_sequence: freshness,
        };
        let mut expected = [0; 4096];
        let length = serde_json_core::to_slice(&owned, &mut expected).unwrap();
        let borrowed = BorrowedArrival {
            protocol: PICO_ADMISSION_PROTOCOL,
            advertisement: &actual,
            friendly_label: label,
            verifying_key: &key,
            freshness_sequence: freshness,
        };
        let mut output = [0; 4096];
        let (written, cost) = allocation_probe::observe(|| {
            serde_json_core::to_slice(&borrowed, &mut output).unwrap()
        });
        assert_eq!(written, length);
        assert_eq!(&output[..written], &expected[..length]);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        let decoded: PicoAdmissionAdvertisement =
            serde_json::from_slice(&output[..written]).unwrap();
        assert_eq!(decoded, owned);
        let mut too_short = [0; 8];
        let (refused, cost) = allocation_probe::observe(|| {
            serde_json_core::to_slice(&borrowed, &mut too_short).is_err()
        });
        assert!(refused);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    }
    // Construction of this view is also a borrow, not another metadata owner.
    let borrowed = BorrowedArrival {
        protocol: PICO_ADMISSION_PROTOCOL,
        advertisement: &actual,
        friendly_label: "Green",
        verifying_key: &key,
        freshness_sequence: 1,
    };
    assert!(core::ptr::eq(borrowed.advertisement, &actual));
    assert!(core::ptr::eq(borrowed.verifying_key, &key));
}
