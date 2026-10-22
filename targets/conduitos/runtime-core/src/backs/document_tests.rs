//! Numeric production-kernel regression, not native service/platform proof.
use crate::admission::{Admission, BackKind};
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec},
    *,
};

#[test]
fn initialized_document_reaches_output_before_any_edit_host_request() {
    let mut admission = Admission::new(3, 2);
    admission.nodes[0].maximum_step_fuel = 16;
    admission.nodes[1].maximum_step_fuel = 16;
    admission.nodes[2].maximum_step_fuel = 16;
    admission.backs[2] = BackKind::Keyboard;
    admission.backs[0] = BackKind::Document;
    admission.backs[1] = BackKind::Face;
    admission.nodes[0].input_cords[0] = Some(CordId(0));
    admission.nodes[1].input_cords[0] = Some(CordId(1));
    admission.cords[0] = CordSpec::local(
        CordId(0),
        (NodeId(2), PortId(0)),
        (NodeId(0), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: 272,
            pressure_policy: Default::default(),
        },
    )
    .with_maximum_value_bytes(272);
    admission.cords[1] = CordSpec::local(
        CordId(1),
        (NodeId(0), PortId(0)),
        (NodeId(1), PortId(0)),
        CordCapacity {
            slot_start: 1,
            item_capacity: 1,
            byte_capacity: 264,
            pressure_policy: Default::default(),
        },
    )
    .with_maximum_value_bytes(264);
    admission
        .install_route(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    admission.bindings[0] = Some(HostCallBinding {
        call: HostCallId(0),
        maximum_input_bytes: 272,
        maximum_output_bytes: 264,
    });
    admission.bindings[1] = Some(HostCallBinding {
        call: HostCallId(0),
        maximum_input_bytes: 264,
        maximum_output_bytes: 0,
    });
    admission
        .install_route(
            NodeId(2),
            PortId(0),
            RouteRange { start: 1, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(0), PortId(0)),
            }],
        )
        .unwrap();
    admission.bindings[2] = Some(HostCallBinding {
        call: HostCallId(0),
        maximum_input_bytes: 0,
        maximum_output_bytes: 272,
    });
    let mut scheduler = admission.build().unwrap();
    assert!(scheduler.next_host_request().is_none());
    let shown = (0..16)
        .find_map(|_| {
            scheduler.step().unwrap();
            scheduler.next_host_request()
        })
        .expect("bounded production steps yield the exact Host Call");
    assert_eq!(shown.node, NodeId(1));
    assert_eq!(scheduler.host_value(shown.input.value).unwrap(), &[0; 8]);
    scheduler
        .complete_host_call(
            shown.node,
            shown.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    // A real empty insertion still carries a full 16-byte canonical edit header.
    let keyboard = (0..16)
        .find_map(|_| {
            scheduler.step().unwrap();
            scheduler.next_host_request()
        })
        .expect("prepared edit source request");
    assert_eq!(keyboard.node, NodeId(2));
    let value = scheduler.store_host_value(&[0; 16]).unwrap();
    scheduler
        .complete_host_call(
            keyboard.node,
            keyboard.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value, 272).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let edit = (0..16)
        .find_map(|_| {
            scheduler.step().unwrap();
            scheduler.next_host_request()
        })
        .expect("bounded production steps yield the exact Host Call");
    assert_eq!(edit.node, NodeId(0));
    assert_eq!(
        edit.request,
        RequestId(0),
        "initial publication consumes no request ID"
    );
    assert_eq!(scheduler.host_value(edit.input.value).unwrap(), &[0; 16]);
    scheduler.cancel().unwrap();
}

#[test]
fn additive_document_kind_roundtrips_without_reinterpreting_legacy_variants() {
    let mut admission = Admission::new(1, 0);
    admission.backs[..7].copy_from_slice(&[
        BackKind::Keyboard,
        BackKind::Keymap,
        BackKind::Upper,
        BackKind::Edit,
        BackKind::Face,
        BackKind::Application,
        BackKind::Document,
    ]);
    let mut bytes = [0; 16 * 1024];
    let length = crate::wire::admission::encode(&admission, &mut bytes).unwrap();
    let decoded = crate::wire::admission::decode(&bytes[..length]).unwrap();
    assert_eq!(decoded.backs, admission.backs);
}
