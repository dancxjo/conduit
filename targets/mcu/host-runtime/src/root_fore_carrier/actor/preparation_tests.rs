//! BOTH sessions and exact original bridge policy fail before admission/IO.
use super::*;
#[test]
fn actual_frame_extent_and_input_abi_and_original_policy_are_prestart_gates() {
    for case in 0..4 {
        let mut f = documents::Fixture::new();
        f.host.profile = "config/1".into();
        let seed = root_test::seed(&f);
        let mut lines = [line(false), line(true)];
        if case == 0 {
            lines[0].binding.limits.maximum_frame_bytes = 59;
        }
        let pins = [pin(&lines[0], false), pin(&lines[1], true)];
        let n = numeric(&pins);
        let inventory: Vec<u8> = pins
            .iter()
            .flat_map(|p| p.fore().payload().iter().copied())
            .collect();
        let document = documents::document(&n.bytes, "body/1", "boot/1", &inventory, &[]);
        let root = root_test::root(&f, &seed, &document);
        let peer = RefCell::new(None);
        let mut transport = TestTransport(&peer);
        let mut frame = [0; 128];
        let mut input = [0; 8];
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
                bounds(),
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
        let mut ingress = PrimitiveIngressSpec {
            pin: &pins[0],
            value_kind: c::COUNT_INFO_ID,
            temporal: "flow-open",
            quantity_profile: c::QuantityValueProfile::Decimal,
            item_capacity: 1,
            byte_capacity: 8,
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
            ingress.byte_capacity = 9;
        }
        match carrier.prepare_bridge_sessions(&bridge, ingress, egress) {
            Err(ForeIoRefusal::Bounds) if case == 0 => {}
            Err(ForeIoRefusal::Scope) if case == 1 || case == 2 => {}
            Ok(sessions) if case == 3 => {
                assert!(!sessions.routes[0].machine.is_active());
                assert!(!sessions.routes[1].machine.is_active());
            }
            _ => panic!("wrong preflight outcome"),
        }
        assert!(peer.borrow().is_none());
        assert!(root.resources.admissions().is_empty());
    }
}
