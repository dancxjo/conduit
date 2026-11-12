extern crate std;
use super::*;
use crate::execution_workset::{AssignedPreparationProfile, PreparedLocalWorkset};
use core::cell::Cell;
use sha2::{Digest, Sha256};
use std::{vec, vec::Vec};
#[allow(dead_code)]
#[path = "test_support.rs"]
mod docs;
const PREP: b::HostPreparationDelegation = b::HostPreparationDelegation {
    authorizer: b::HostPreparationAuthorizer::SeedOwner,
    maximum_assignment_bytes: 4096,
    maximum_runtime_state_bytes: 1024,
};
const EXEC: b::HostExecutionDelegation = b::HostExecutionDelegation {
    authorizer: b::HostExecutionAuthorizer::SeedOwner,
    maximum_active_worksets: 1,
    continuation: None,
};
struct Mock<'a>(&'a Cell<usize>);
impl Carrier for Mock<'_> {
    type Error = ();
    async fn connected(&mut self) {
        self.0.set(self.0.get() + 1);
    }
    async fn read(&mut self, _: &mut [u8], _: u32) -> Result<usize, ()> {
        self.0.set(self.0.get() + 1);
        Err(())
    }
    async fn write(&mut self, _: &[u8], _: u32) -> Result<(), ()> {
        self.0.set(self.0.get() + 1);
        Err(())
    }
}
struct Ticket<'a>(&'a Cell<usize>);
// SAFETY: inert CPU-only ticket, created after exact opaque scope validation;
// bind counts the original transfer and has no native effects or allocation.
unsafe impl PreparedLocalKernels for Ticket<'_> {
    type Bound = ();
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    unsafe fn bind(self) {
        self.0.set(self.0.get() + 1);
    }
}
fn bounds() -> ForeCarrierBounds {
    ForeCarrierBounds {
        maximum_lines: 1,
        maximum_frame_bytes: 64,
        maximum_payload_bytes: 64,
        maximum_buffered_bytes: 64,
        maximum_in_flight_items: 1,
        maximum_frames: 8,
    }
}
fn actual_line() -> c::AdmittedLine {
    c::AdmittedLine {
        line_id: "line".into(),
        binding: c::BoundLink {
            binding_id: "binding".into(),
            source: c::LinkEndpoint {
                host_id: "issuer/1".into(),
                boot_id: "issuer/boot/1".into(),
                endpoint_id: "owner/read_index".into(),
            },
            sink: c::LinkEndpoint {
                host_id: "host/1".into(),
                boot_id: "boot/1".into(),
                endpoint_id: "member/read_index".into(),
            },
            base: "serial/base".into(),
            base_instance_id: "serial/actual".into(),
            credential: c::LinkCredentialReference::None,
            authority: c::LinkAuthorityReference::ProcessOwned,
            limits: c::LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 64,
            },
        },
        contract: c::LineContract {
            scope: c::LineScope::PointToPoint,
            traffic_shape: c::LineTrafficShape::Message,
            duplex: c::LineDuplex::FullDuplex,
            ordering: c::LineOrdering::Ordered,
            reliability: c::LineReliability::Reliable,
            continuation: c::LineContinuation::None,
            security: c::LineSecurity::PhysicalPossession,
        },
    }
}
fn binding() -> c::AssignedRemoteBinding {
    c::AssignedRemoteBinding {
        line: docs::id("line"),
        local_host: docs::id("host/1"),
        local_boot: docs::id("boot/1"),
        peer_host: docs::id("issuer/1"),
        peer_boot: docs::id("issuer/boot/1"),
    }
}
fn pin(line: &c::AdmittedLine) -> c::AssignedLineForeRequirement {
    c::AssignedLineForeRequirement::new(
        docs::remote(),
        binding(),
        docs::id("session"),
        docs::id("serial-profile"),
        c::admitted_line_commitment(line),
    )
}
fn document(pin: &c::AssignedLineForeRequirement) -> Vec<u8> {
    let mut bytes = docs::projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let mut at = 125;
    while at < bytes.len() {
        let n = usize::from(u16::from_le_bytes([bytes[at + 1], bytes[at + 2]]));
        if bytes[at] == c::ASSIGNED_REMOTE_ENDPOINT && bytes[at + 3] == 1 {
            bytes[at + 3..at + 3 + n].copy_from_slice(pin.payload());
        }
        at += 3 + n;
    }
    let digest = c::assigned_plan_payload_digest(&bytes[125..]);
    bytes[93..125].copy_from_slice(&digest);
    docs::document(&bytes, "body/1", "boot/1", &docs::inventory(), &[])
}
fn fixture() -> docs::Fixture {
    let mut f = docs::Fixture::new();
    f.host.profile = "config/1".into();
    f
}
fn seed(f: &docs::Fixture) -> Vec<u8> {
    let payload = serde_json::to_vec(&f.bootstrap).unwrap();
    let mut bytes = b"CONDUIT_SEED@2\0\0".to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}
fn root<'s>(f: &docs::Fixture, seed: &'s [u8], document: &[u8]) -> Root<'s, 4096> {
    let mut root = Root::new(
        crate::seed::Seed::parse(seed, "host/1", "image/1", "config/1").unwrap(),
        f.host.clone(),
        Configuration {
            image_id: "image/1",
            configuration_id: "config/1",
            friendly_label: "fixture",
            preparation: Some(&PREP),
            execution: Some(&EXEC),
        },
    )
    .unwrap();
    root.receipt = Some(f.member.clone());
    let payload = b::encode_body_workset_document(&[document], 4096).unwrap();
    root.input.writable().unwrap()[..payload.len()].copy_from_slice(&payload);
    root.input
        .retain(
            docs::workset_command(f, &payload)
                .into_retained_stamp()
                .unwrap(),
        )
        .unwrap();
    root
}
#[test]
fn current_root_command_policy_peer_line_session_and_bounds_are_independent_gates() {
    let f = fixture();
    let seed = seed(&f);
    let original = [actual_line()];
    let pins = [pin(&original[0])];
    let local = [docs::local()];
    let bindings = [binding()];
    let document = document(&pins[0]);
    let root = root(&f, &seed, &document);
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
    let set = b::receive_delegated_body_workset::<1>(Some(&PREP), context, command).unwrap();
    let (approved, fore) = set
        .receive_fragment_with_line_fore::<2>(
            0,
            Some(&PREP),
            context,
            docs::profile(&local, &pins, &bindings),
        )
        .unwrap();
    assert!(matches!(
        root.prepare_fore_carrier::<Mock<'_>, 1, 2>(
            11,
            None,
            &approved,
            &fore,
            docs::profile(&local, &pins, &bindings)
        ),
        Err(ForeCarrierRefusal::MissingSelection)
    ));
    let io = Cell::new(0);
    let changed_abi = [c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            1,
            6,
            c::AssignedForeDirection::Input,
            docs::id("read_index"),
            [99; 32],
        ),
        binding(),
        docs::id("session"),
        docs::id("serial-profile"),
        c::admitted_line_commitment(&original[0]),
    )];
    for change in 0..9 {
        let mut link = Link::new(Mock(&io));
        let mut backing = [0; 64];
        let mut lines = original.clone();
        let mut peer = f.issuer.clone();
        if change == 3 {
            peer.boot_id = "foreign/boot".into();
        }
        if change == 4 {
            lines[0].binding.limits.maximum_payload_bytes = 63;
        }
        if change == 6 {
            lines[0].binding.sink.boot_id = "foreign/boot".into();
        }
        let selection = unsafe {
            NativeForeCarrierSelection::from_native_custody(
                &mut link,
                &mut backing,
                if change == 1 {
                    "foreign-image"
                } else {
                    "image/1"
                },
                if change == 2 {
                    "foreign-config"
                } else {
                    "config/1"
                },
                &peer,
                c::AssignedLineForeProfile {
                    local: &local,
                    lines: &pins,
                },
                &lines,
                if change == 5 {
                    "foreign-session"
                } else {
                    "session"
                },
                "serial-profile",
                bounds(),
            )
        }
        .unwrap();
        let result = root.prepare_fore_carrier::<Mock<'_>, 1, 2>(
            if change == 0 { 30 } else { 11 },
            Some(selection),
            &approved,
            &fore,
            docs::profile(
                &local,
                if change == 7 {
                    &[]
                } else if change == 8 {
                    &changed_abi
                } else {
                    &pins
                },
                &bindings,
            ),
        );
        assert!(result.is_err());
    }
    let mut link = Link::new(Mock(&io));
    let mut short = [0; 63];
    assert!(unsafe {
        NativeForeCarrierSelection::from_native_custody(
            &mut link,
            &mut short,
            "image/1",
            "config/1",
            &f.issuer,
            c::AssignedLineForeProfile {
                local: &local,
                lines: &pins,
            },
            &original,
            "session",
            "serial-profile",
            bounds(),
        )
    }
    .is_err());
    assert_eq!(io.get(), 0);
}

#[test]
fn another_valid_child_or_body_and_missing_root_policy_cannot_supply_carrier_authority() {
    let f = fixture();
    let seed = seed(&f);
    let lines = [actual_line()];
    let pins = [pin(&lines[0])];
    let local = [docs::local()];
    let bindings = [binding()];
    let original = document(&pins[0]);
    let mut root = root(&f, &seed, &original);
    let (metadata, projection) = b::decode_body_fragment_document(&original).unwrap();
    let mut changed = projection.to_vec();
    changed[32..48].copy_from_slice(&docs::id("fragment/foreign").0);
    let foreign = docs::document_for(
        &changed,
        "body/1",
        "boot/1",
        metadata.fore_ports,
        &[],
        "fragment/foreign",
        &["fragment/foreign"],
    );
    let (approved, fore) = b::receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&PREP),
        f.context(),
        docs::command(&f, &foreign),
        docs::profile(&local, &pins, &bindings),
    )
    .unwrap();
    let io = Cell::new(0);
    let mut link = Link::new(Mock(&io));
    let mut backing = [0; 64];
    let selection = unsafe {
        NativeForeCarrierSelection::from_native_custody(
            &mut link,
            &mut backing,
            "image/1",
            "config/1",
            &f.issuer,
            c::AssignedLineForeProfile {
                local: &local,
                lines: &pins,
            },
            &lines,
            "session",
            "serial-profile",
            bounds(),
        )
    }
    .unwrap();
    assert!(matches!(
        root.prepare_fore_carrier::<Mock<'_>, 1, 2>(
            11,
            Some(selection),
            &approved,
            &fore,
            docs::profile(&local, &pins, &bindings)
        ),
        Err(ForeCarrierRefusal::Scope)
    ));
    let mut other = fixture();
    other.member.body_id = serde_json::from_value(serde_json::json!("body/other")).unwrap();
    other.bootstrap.body_id = other.member.body_id.clone();
    other.bootstrap.admission = b::MemberBootstrapMaterial::Retained {
        credential: other.member.clone(),
        secret: [3; 32],
    };
    let foreign = docs::document(projection, "body/other", "boot/1", metadata.fore_ports, &[]);
    let (approved, fore) = b::receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&PREP),
        other.context(),
        docs::command(&other, &foreign),
        docs::profile(&local, &pins, &bindings),
    )
    .unwrap();
    let mut link = Link::new(Mock(&io));
    let mut backing = [0; 64];
    let selection = unsafe {
        NativeForeCarrierSelection::from_native_custody(
            &mut link,
            &mut backing,
            "image/1",
            "config/1",
            &f.issuer,
            c::AssignedLineForeProfile {
                local: &local,
                lines: &pins,
            },
            &lines,
            "session",
            "serial-profile",
            bounds(),
        )
    }
    .unwrap();
    assert!(matches!(
        root.prepare_fore_carrier::<Mock<'_>, 1, 2>(
            11,
            Some(selection),
            &approved,
            &fore,
            docs::profile(&local, &pins, &bindings)
        ),
        Err(ForeCarrierRefusal::Scope)
    ));
    let (approved, fore) = b::receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&PREP),
        f.context(),
        docs::command(&f, &original),
        docs::profile(&local, &pins, &bindings),
    )
    .unwrap();
    root.config.preparation = None;
    let mut link = Link::new(Mock(&io));
    let mut backing = [0; 64];
    let selection = unsafe {
        NativeForeCarrierSelection::from_native_custody(
            &mut link,
            &mut backing,
            "image/1",
            "config/1",
            &f.issuer,
            c::AssignedLineForeProfile {
                local: &local,
                lines: &pins,
            },
            &lines,
            "session",
            "serial-profile",
            bounds(),
        )
    }
    .unwrap();
    assert!(matches!(
        root.prepare_fore_carrier::<Mock<'_>, 1, 2>(
            11,
            Some(selection),
            &approved,
            &fore,
            docs::profile(&local, &pins, &bindings)
        ),
        Err(ForeCarrierRefusal::Workset)
    ));
    assert_eq!(io.get(), 0);
}

#[path = "start_tests.rs"]
mod start;
