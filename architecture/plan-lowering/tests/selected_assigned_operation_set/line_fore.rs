//! Actual original opaque operations under explicit schema-8 preparation.
use super::*;
fn pins(f: &Fixture) -> (AssignedLineForeRequirement, AssignedRemoteBinding) {
    let binding = AssignedRemoteBinding {
        line: AssignedIdentity::from_text("line"),
        local_host: AssignedIdentity::from_text(f.fragment.host_id.as_str()),
        local_boot: AssignedIdentity::from_text(f.fragment.boot_id.as_str()),
        peer_host: AssignedIdentity::from_text("peer"),
        peer_boot: AssignedIdentity::from_text("peer-boot"),
    };
    (
        AssignedLineForeRequirement::new(
            AssignedForeRequirement::new(
                0,
                0,
                AssignedForeDirection::Input,
                AssignedIdentity::from_text("read_index"),
                [8; 32],
            ),
            binding,
            AssignedIdentity::from_text("session"),
            AssignedIdentity::from_text("session-profile"),
            [7; 32],
        ),
        binding,
    )
}
fn schema8(mut bytes: Vec<u8>, pin: &AssignedLineForeRequirement) -> Vec<u8> {
    bytes[8..10].copy_from_slice(&ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA.to_le_bytes());
    bytes[89] = 1;
    bytes.push(ASSIGNED_REMOTE_ENDPOINT);
    bytes.extend_from_slice(&(pin.payload().len() as u16).to_le_bytes());
    bytes.extend_from_slice(pin.payload());
    let len = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&len.to_le_bytes());
    let digest = assigned_plan_payload_digest(&bytes[125..]);
    bytes[93..125].copy_from_slice(&digest);
    bytes
}
#[test]
fn line_service_reuses_original_possession_and_all_target_identity_checks() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let r = operation(f.scopes[1].clone());
    let (pin, binding) = pins(&f);
    let lines = [pin];
    let bindings = [binding];
    let profile = AssignedLineForeProfile {
        local: &[],
        lines: &lines,
    };
    let bytes = schema8(project(&f, |_, _| {}), &pin);
    let before = [a.inspection().clone(), r.inspection().clone()];
    assert!(check_mode(&f, &bytes, &[&a, &r], false, Some((profile, &bindings))).is_ok());
    assert_eq!([a.inspection().clone(), r.inspection().clone()], before);
    assert!(check(&f, &bytes, &[&a, &r], false).is_err());
    assert!(check_mode(&f, &bytes, &[&a, &r], true, Some((profile, &bindings))).is_err());
    let foreign = [AssignedLineForeRequirement::new(
        *pin.fore(),
        binding,
        AssignedIdentity::from_text("foreign-session"),
        AssignedIdentity::from_text("session-profile"),
        [7; 32],
    )];
    assert!(check_mode(
        &f,
        &bytes,
        &[&a, &r],
        false,
        Some((
            AssignedLineForeProfile {
                local: &[],
                lines: &foreign
            },
            &bindings
        ))
    )
    .is_err());
    for change in 0..4 {
        let bytes = schema8(
            project(&f, |prefix, binding| match change {
                0 => binding.grant = AssignedIdentity::from_text("foreign-grant"),
                1 => {
                    prefix[20..36].copy_from_slice(&AssignedIdentity::from_text("foreign-target").0)
                }
                2 => binding.pool = AssignedIdentity::from_text("foreign-pool"),
                _ => binding.authority = AssignedIdentity::from_text("foreign-authority"),
            }),
            &pin,
        );
        assert!(check_mode(&f, &bytes, &[&a, &r], false, Some((profile, &bindings))).is_err());
    }
    let mut scope = f.scopes[1].clone();
    scope.boot_id = "foreign-boot".into();
    let foreign = operation(scope);
    assert!(check_mode(
        &f,
        &bytes,
        &[&a, &foreign],
        false,
        Some((profile, &bindings))
    )
    .is_err());
}
