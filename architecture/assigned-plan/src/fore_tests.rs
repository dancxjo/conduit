//! Local Fore cannot manufacture a network binding or enter legacy admission.
use crate::*;
use std::{vec, vec::Vec};
fn requirements() -> AssignedPlanRequirements<'static> {
    AssignedPlanRequirements {
        host: AssignedIdentity::from_text("fixture/host"),
        boot: AssignedIdentity::from_text("fixture/boot"),
        host_calls: &[],
        resources: &[],
        remote_bindings: &[],
    }
}
fn computations() -> AssignedComputationProfile<'static> {
    AssignedComputationProfile {
        maximum_text_bytes: 16,
        maximum_total_text_bytes: 16,
        requirements: &[],
    }
}
fn fore() -> [AssignedForeRequirement; 2] {
    [
        AssignedForeRequirement::new(
            0,
            0,
            AssignedForeDirection::Input,
            AssignedIdentity::from_text("frame"),
            [1; 32],
        ),
        AssignedForeRequirement::new(
            1,
            4,
            AssignedForeDirection::Output,
            AssignedIdentity::from_text("invalid_brightness"),
            [2; 32],
        ),
    ]
}
fn envelope(payloads: &[Vec<u8>], schema: u16) -> Vec<u8> {
    let mut bytes = vec![0; ASSIGNED_PLAN_HEADER_BYTES];
    bytes[..8].copy_from_slice(&assigned_plan_magic());
    bytes[8..10].copy_from_slice(&schema.to_le_bytes());
    bytes[12..14].copy_from_slice(&128u16.to_le_bytes());
    let required = requirements();
    bytes[48..64].copy_from_slice(&required.host.0);
    bytes[64..80].copy_from_slice(&required.boot.0);
    bytes[80 + 9] = payloads.len() as u8;
    for payload in payloads {
        bytes.push(ASSIGNED_LOCAL_FORE);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    let len = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&len.to_le_bytes());
    let digest = assigned_plan_payload_digest(&bytes[ASSIGNED_PLAN_HEADER_BYTES..]);
    bytes[93..125].copy_from_slice(&digest);
    bytes
}
fn admit(
    bytes: &[u8],
    expected: &[AssignedForeRequirement],
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    let records = AssignedPlanRecords::new(bytes)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let (borrowed, memory) = crate::record_view_tests::allocation_probe::observe(|| {
        decode_assigned_records_with_fore(
            &bytes[..125],
            &records,
            AssignedPlanMaxima::TINY_HOST,
            requirements(),
            computations(),
            expected,
        )
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let (contiguous, memory) = crate::record_view_tests::allocation_probe::observe(|| {
        decode_assigned_plan_with_fore(
            bytes,
            AssignedPlanMaxima::TINY_HOST,
            requirements(),
            computations(),
            expected,
        )
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(borrowed, contiguous);
    contiguous
}
#[test]
fn exact_local_fore_is_opt_in_current_and_allocation_free() {
    let fore = fore();
    let payloads: Vec<_> = fore.iter().map(|r| r.payload().to_vec()).collect();
    let bytes = envelope(&payloads, ASSIGNED_FORE_NATIVE_PLAN_SCHEMA);
    assert_eq!(admit(&bytes, &fore).unwrap().counts[9], 2);
    assert_eq!(fore[0].direction(), AssignedForeDirection::Input);
    assert_eq!((fore[1].endpoint(), fore[1].cord()), (1, 4));
    assert_eq!(
        decode_assigned_plan_with_computations(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            requirements(),
            computations()
        ),
        Err(AssignedPlanRefusal::WrongSchema)
    );
    assert_eq!(
        decode_assigned_records_with_computations(
            &bytes[..125],
            &AssignedPlanRecords::new(&bytes)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            AssignedPlanMaxima::TINY_HOST,
            requirements(),
            computations()
        ),
        Err(AssignedPlanRefusal::WrongSchema)
    );
    let mut required = requirements();
    required.boot = AssignedIdentity::from_text("foreign/boot");
    assert_eq!(
        decode_assigned_plan_with_fore(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            required,
            computations(),
            &fore
        ),
        Err(AssignedPlanRefusal::WrongBoot)
    );
    required = requirements();
    required.host = AssignedIdentity::from_text("foreign/host");
    assert_eq!(
        decode_assigned_plan_with_fore(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            required,
            computations(),
            &fore
        ),
        Err(AssignedPlanRefusal::WrongHost)
    );
    assert_eq!(
        admit(&envelope(&payloads, ASSIGNED_NATIVE_PLAN_SCHEMA), &fore),
        Err(AssignedPlanRefusal::WrongSchema)
    );
}
#[test]
fn altered_duplicate_missing_and_network_records_do_not_admit_fore() {
    let expected = fore();
    let payloads: Vec<_> = expected.iter().map(|r| r.payload().to_vec()).collect();
    for byte in [0, 2, 5, 21, 52] {
        let mut changed = payloads.clone();
        changed[0][byte] ^= 1;
        assert_eq!(
            admit(&envelope(&changed, 7), &expected),
            Err(AssignedPlanRefusal::UnknownForeEndpoint)
        );
    }
    let mut changed = payloads.clone();
    changed[0][4] = 2;
    assert_eq!(
        admit(&envelope(&changed, 7), &expected),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
    assert_eq!(
        admit(
            &envelope(&[payloads[0].clone(), payloads[0].clone()], 7),
            &expected
        ),
        Err(AssignedPlanRefusal::UnknownForeEndpoint)
    );
    assert_eq!(
        admit(&envelope(&payloads[..1], 7), &expected),
        Err(AssignedPlanRefusal::MissingForeEndpoint)
    );
    assert_eq!(
        admit(&envelope(&[vec![0; 250]], 7), &expected),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
    assert_eq!(
        admit(&envelope(&payloads, 7), &[expected[0], expected[0]]),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
    let line = [AssignedRemoteBinding {
        line: AssignedIdentity([0; 16]),
        local_host: requirements().host,
        local_boot: requirements().boot,
        peer_host: AssignedIdentity([0; 16]),
        peer_boot: AssignedIdentity([0; 16]),
    }];
    let mut required = requirements();
    required.remote_bindings = &line;
    assert_eq!(
        decode_assigned_plan_with_fore(
            &envelope(&payloads, 7),
            AssignedPlanMaxima::TINY_HOST,
            required,
            computations(),
            &expected
        ),
        Err(AssignedPlanRefusal::WrongSchema)
    );
}
