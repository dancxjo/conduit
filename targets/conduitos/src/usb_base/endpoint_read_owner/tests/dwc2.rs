use super::{fixture, *};
use crate::{
    arch::dwc2::{EndpointRoute, PacketError, TransactionTranslator, TransferType, test_io},
    usb_base::dwc2_endpoint_read::*,
};
use alloc::vec::Vec;
fn route() -> RootEndpointRoute {
    RootEndpointRoute {
        slot: 1,
        generation: 7,
        endpoint_generation: 9,
        endpoint: EndpointRoute {
            address: 2,
            endpoint: 1,
            maximum_packet: 8,
            transfer_type: TransferType::Interrupt,
            translator: Some(TransactionTranslator {
                hub_address: 1,
                port: 2,
                low_speed: false,
            }),
        },
    }
}
fn owner() -> EndpointReadCallOwner {
    let mut owner = fixture::owner(7);
    owner.attachment.maximum_data_bytes = 8;
    owner
}
fn stage(status: u32) -> (u32, Vec<u32>, Vec<u32>) {
    (status, vec![], vec![])
}
#[test]
fn dwc2_nak_does_not_advance_report_ordinal_or_data_pid() {
    let owner = owner();
    let node = owner.binding_node();
    let mut controller = test_io::prepared();
    let io = test_io::registers(&mut controller);
    io.clock_step = 10;
    io.writes.reserve(256);
    io.script.extend([
        stage(0x22),
        stage(0x12),
        stage(0x22),
        (3, vec![(2 << 17) | (3 << 4)], vec![0x030201]),
    ]);
    let mut host = unsafe { Dwc2EndpointReadHost::bind_owned(controller, owner, route()) }.unwrap();
    let request = fixture::input_length(8);
    let result = host
        .execute(node, HostCallId(0), RequestId(0), &request)
        .unwrap();
    assert!(
        conduit_core::validate_canonical_structured_value(result)
            .unwrap()
            .variant_payload("timeout")
            .unwrap()
            .is_some()
    );
    let allocations = crate::test_allocations::allocations(|| {
        let result = host
            .execute(node, HostCallId(0), RequestId(1), &request)
            .unwrap();
        let completed = conduit_core::validate_canonical_structured_value(result)
            .unwrap()
            .variant_payload("completed")
            .unwrap()
            .unwrap();
        assert_eq!(
            completed
                .record_field("ordinal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            0_u64.to_le_bytes()
        );
        assert_eq!(
            completed
                .record_field("actual")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            3_u64.to_le_bytes()
        );
    });
    assert_eq!(allocations, 0);
    assert!(!host.retains_submission());
}
#[test]
fn dwc2_endpoint_identity_and_wire_geometry_are_selected_not_request_authority() {
    let mut wrong = route();
    wrong.endpoint_generation = 10;
    assert!(matches!(
        unsafe { Dwc2EndpointReadHost::bind_owned(test_io::prepared(), owner(), wrong) },
        Err(Dwc2ReadRefusal::Binding)
    ));
    let selected = owner();
    let node = selected.binding_node();
    let mut host =
        unsafe { Dwc2EndpointReadHost::bind_owned(test_io::prepared(), selected, route()) }
            .unwrap();
    let result = host
        .execute(node, HostCallId(0), RequestId(0), &fixture::input_length(7))
        .unwrap();
    assert!(
        conduit_core::validate_canonical_structured_value(result)
            .unwrap()
            .variant_payload("unsupported")
            .unwrap()
            .is_some()
    );
    assert!(!host.retains_submission());
}
#[test]
fn dwc2_pending_tt_cannot_be_discarded_by_revocation_or_channel_halt() {
    let selected = owner();
    let node = selected.binding_node();
    let mut controller = test_io::prepared();
    let io = test_io::registers(&mut controller);
    io.clock_step = 10;
    io.script
        .extend([stage(0x22), stage(0x42), stage(0x42), stage(0x42)]);
    let mut host =
        unsafe { Dwc2EndpointReadHost::bind_owned(controller, selected, route()) }.unwrap();
    assert_eq!(
        host.execute(node, HostCallId(0), RequestId(0), &fixture::input_length(8)),
        Err(Dwc2ReadRefusal::Native(PacketError::Retirement))
    );
    host.revoke(node, HostCallId(0)).unwrap();
    assert_eq!(
        host.discard_retired(),
        Err(Dwc2ReadRefusal::Native(PacketError::Retirement))
    );
    assert!(host.retains_submission());
}
