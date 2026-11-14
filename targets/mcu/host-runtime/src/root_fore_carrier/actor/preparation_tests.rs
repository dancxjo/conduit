//! BOTH sessions and exact original bridge policy fail before admission/IO.
use super::*;
#[test]
fn actual_frame_extent_and_input_abi_and_original_policy_are_prestart_gates() {
    for case in 0..5 {
        let mut f = documents::Fixture::new();
        f.host.profile = "config/1".into();
        let seed = root_test::seed(&f);
        let mut lines = [line(false), line(true)];
        if case == 0 {
            lines[0].binding.limits.maximum_frame_bytes = 59;
        }
        let input_capacity = if case == 4 { 128 } else { 8 };
        let mut carrier_bounds = bounds();
        carrier_bounds.maximum_buffered_bytes = 256;
        if case == 4 {
            lines[0].binding.limits.maximum_payload_bytes = input_capacity;
            lines[0].binding.limits.maximum_buffered_bytes = input_capacity;
            lines[0].binding.limits.maximum_frame_bytes = 256;
            carrier_bounds.maximum_payload_bytes = input_capacity;
            carrier_bounds.maximum_frame_bytes = 256;
            carrier_bounds.maximum_buffered_bytes = 256;
        }
        let schema = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
        let structured_kind = schema.profile().unwrap().value_kind().clone();
        let kind = if case == 4 {
            structured_kind.as_str()
        } else {
            c::COUNT_INFO_ID
        };
        let pins = [
            pin_with_abi(&lines[0], false, kind, input_capacity),
            pin(&lines[1], true),
        ];
        let n = numeric(&pins);
        let inventory: Vec<u8> = pins
            .iter()
            .flat_map(|p| p.fore().payload().iter().copied())
            .collect();
        let document = documents::document(&n.bytes, "body/1", "boot/1", &inventory, &[]);
        let root = root_test::root(&f, &seed, &document);
        let peer = RefCell::new(None);
        let mut transport = TestTransport(&peer);
        let mut frame = [0; 256];
        let mut input = [0; 128];
        let mut output = [0; 8];
        let selection = unsafe {
            NativeForeCarrierSelection::from_native_custody(
                &mut transport,
                &mut frame,
                "image/1",
                "config/1",
                &f.issuer,
                n.profile(),
                &lines,
                "session",
                "serial-profile",
                carrier_bounds,
            )
        }
        .unwrap();
        let context = root.return_context(11).unwrap();
        let command = root
            .input
            .reborrow(
                context.member,
                context.actual,
                &context.bootstrap.owner,
                b::HOST_PREPARED_WORKSET_CONTRACT,
                11,
            )
            .unwrap();
        let set = b::receive_delegated_body_workset::<1>(Some(&root_test::PREP), context, command)
            .unwrap();
        let (approved, fore) = set
            .receive_fragment_with_line_fore::<2>(
                0,
                Some(&root_test::PREP),
                context,
                n.preparation(),
            )
            .unwrap();
        let carrier = root
            .prepare_fore_carrier::<TestTransport<'_>, 1, 2>(
                11,
                Some(selection),
                &approved,
                &fore,
                n.preparation(),
            )
            .unwrap();
        let abi = c::AssignedForeAbi {
            value_kind: kind,
            value_contract: None,
            abnormal_kind: None,
            temporal: "flow-open",
            track: c::ConnectionTrack::Payload,
            quantity_profile: c::QuantityValueProfile::Decimal,
            item_capacity: 1,
            byte_capacity: input_capacity,
        };
        let ingress: crate::fore_bridge::IngressSpec<'_> = if case == 4 {
            // This proves original Root route preparation, not payload delivery:
            // the structured route uses an explicitly larger admitted bound.
            let schema =
                std::boxed::Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice());
            StructuredIngressSpec::checked(&pins[0], abi, schema)
                .unwrap()
                .into()
        } else {
            PrimitiveIngressSpec::checked(&pins[0], abi).unwrap().into()
        };
        let egress = EgressSpec::checked(
            &pins[1],
            c::AssignedForeAbi {
                value_kind: c::COUNT_INFO_ID,
                value_contract: None,
                abnormal_kind: None,
                temporal: "flow-open",
                track: c::ConnectionTrack::Payload,
                quantity_profile: c::QuantityValueProfile::Decimal,
                item_capacity: 1,
                byte_capacity: 8,
            },
        )
        .unwrap();
        let mut custody = *carrier.custody_digest();
        if case == 2 {
            custody[0] ^= 1;
        }
        let bridge =
            PreparedForeBridge::prepare(custody, ingress, egress, &mut input, &mut output).unwrap();
        if case == 1 {
            assert!(matches!(
                PrimitiveIngressSpec::checked(
                    &pins[0],
                    c::AssignedForeAbi {
                        value_kind: c::COUNT_INFO_ID,
                        value_contract: None,
                        abnormal_kind: None,
                        temporal: "flow-open",
                        track: c::ConnectionTrack::Payload,
                        quantity_profile: c::QuantityValueProfile::Decimal,
                        item_capacity: 1,
                        byte_capacity: 9,
                    }
                ),
                Err(crate::fore_bridge::Refusal::UnsupportedAbi)
            ));
            assert!(peer.borrow().is_none());
            assert!(root.resources.admissions().is_empty());
            continue;
        }
        match carrier.prepare_bridge_sessions(&bridge, ingress, egress) {
            Err(ForeIoRefusal::Bounds) if case == 0 => {}
            Err(ForeIoRefusal::Scope) if case == 1 || case == 2 => {}
            Ok(sessions) if case == 3 || case == 4 => {
                assert!(!sessions.routes[0].machine.is_active());
                assert!(!sessions.routes[1].machine.is_active());
            }
            _ => panic!("wrong preflight outcome"),
        }
        assert!(peer.borrow().is_none());
        assert!(root.resources.admissions().is_empty());
    }
}
