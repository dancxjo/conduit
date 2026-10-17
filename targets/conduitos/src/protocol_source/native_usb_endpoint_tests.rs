//! Ordinary packaged endpoint-only preparation, Body/Kernel dispatch and native
//! acknowledged cancellation. Synthetic controller/ConfigureEndpoint, no guest proof.
use super::*;
use crate::protocol_source::{NativeProtocolIssuer, ProtocolSourcePackage};
use conduit_body::ResidentPlot;
struct NoI2c;
impl I2cProvider for NoI2c {
    fn transact(
        &mut self,
        _: &crate::i2c_base::I2cTransaction<'_>,
        _: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        panic!("no I2C")
    }
    fn revoke(&mut self) {}
}
struct NoClock;
impl MonotonicDeadlineProvider for NoClock {
    fn poll_until(
        &mut self,
        _: u64,
    ) -> Result<Option<u64>, crate::monotonic_clock::codec::ClockDisposition> {
        panic!("no clock")
    }
    fn revoke(&mut self) {}
}
fn entry() -> PreparedProtocolEntry {
    let package = ProtocolSourcePackage::compile(
        include_str!("../../plots/usb/endpoint-read.conduit").into(),
        &[],
    )
    .unwrap();
    PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package).unwrap(),
        "usb-endpoint-read-call",
    )
    .unwrap()
}
fn session(entry: &PreparedProtocolEntry, a: &BaseCapabilityAuthority) -> BodyLifecycleSession {
    let expanded = &entry.expanded().expanded;
    crate::protocol_test_support::protocol_body_for_resident(
        &ResidentPlot::new(
            expanded.source_document_id.clone(),
            expanded.checked_plot_id.clone(),
        ),
        &a.grant.host_id,
        &a.grant.boot_id,
    )
}
fn advertisement(a: &BaseCapabilityAuthority) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: a.grant.host_id.clone(),
        boot_id: a.grant.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn unchanged_endpoint_source_prepares_dispatches_and_cancels_without_control_possession() {
    crate::arch::ReadyUsbProtocolHost::with_endpoint_fixture(|ready, control, endpoint| {
        let entry = entry();
        let session = session(&entry, &endpoint);
        let host = advertisement(&endpoint);
        let mut play = prepare_native_usb_endpoint_protocol::<NoI2c, NoClock>(
            entry,
            session,
            host,
            ready,
            NativeProtocolIssuer::from_test_authority(control, [71; 32]).unwrap(),
            NativeProtocolIssuer::from_test_authority(endpoint, [72; 32]).unwrap(),
            0,
            1,
            0,
        )
        .unwrap();
        let kernel = play.kernel().unwrap();
        let gears = &kernel.definition().internal_plan.fragments[0].placements;
        assert_eq!(
            gears
                .iter()
                .filter(|g| g.implementation_id.as_str()
                    == crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION)
                .count(),
            1
        );
        assert!(gears.iter().all(|g| g.implementation_id.as_str()
            != crate::usb_base::control_factory::CONTROL_IMPLEMENTATION));
        let fragment = &kernel.definition().internal_plan.fragments[0];
        let placement = &gears
            .iter()
            .find(|g| {
                g.implementation_id.as_str()
                    == crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION
            })
            .unwrap()
            .placement_id;
        let node = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
            .unwrap()
            .identity
            .node_for_placement(placement)
            .unwrap();
        let contract =
            crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
            panic!("request")
        };
        let request = StructuredInfoValue::record(
            contract.request_type().clone(),
            vec![
                StructuredFieldValue::new(
                    fields[0].name(),
                    StructuredInfoValue::leaf(
                        fields[0].value_type().clone(),
                        8u64.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let input = ValuePayload {
            value_kind: contract.kind().inputs[0].value_kind.clone(),
            encoded: request.canonical_bytes().unwrap(),
        };
        play.start().unwrap();
        play.admit_input(&port_id("requests"), 0, &input).unwrap();
        let mut pending = false;
        for _ in 0..16 {
            play.step().unwrap();
            let kernel = play.kernel().unwrap();
            if kernel
                .has_retained_host_call_for_nodes(&kernel.definition().host_id, &[node])
                .unwrap()
            {
                pending = true;
                break;
            }
        }
        assert!(
            pending,
            "actual endpoint Host Call and original Kernel token retained"
        );
        play.cancel().unwrap();
        assert!(play.kernel().is_none());
        assert!(play.admit_input(&port_id("requests"), 1, &input).is_err());
    });
}
#[test]
fn wrong_endpoint_authority_refuses_before_effect_and_retains_original_body() {
    crate::arch::ReadyUsbProtocolHost::with_endpoint_fixture(|ready, control, mut endpoint| {
        let entry = entry();
        let session = session(&entry, &endpoint);
        let original = session.clone();
        let host = advertisement(&endpoint);
        endpoint.base_provider_generation += 1;
        let result = prepare_native_usb_endpoint_protocol::<NoI2c, NoClock>(
            entry,
            session,
            host,
            ready,
            NativeProtocolIssuer::from_test_authority(control, [73; 32]).unwrap(),
            NativeProtocolIssuer::from_test_authority(endpoint, [74; 32]).unwrap(),
            0,
            1,
            0,
        );
        let refusal = result.err().expect("foreign generation refused");
        assert!(matches!(
            refusal.reason,
            NativeProtocolPreparationReason::Possession(_)
        ));
        assert_eq!(refusal.session.evidence(), original.evidence());
    });
}
