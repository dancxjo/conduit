//! Schema-8 representation conformance; no carrier or execution is installed.
use crate::*;
use std::{vec, vec::Vec};

fn requirements<'a>(bindings: &'a [AssignedRemoteBinding]) -> AssignedPlanRequirements<'a> {
    AssignedPlanRequirements {
        host: AssignedIdentity::from_text("line-host"),
        boot: AssignedIdentity::from_text("line-boot"),
        host_calls: &[],
        resources: &[],
        remote_bindings: bindings,
    }
}
fn computations() -> AssignedComputationProfile<'static> {
    AssignedComputationProfile {
        maximum_text_bytes: 0,
        maximum_total_text_bytes: 0,
        requirements: &[],
    }
}
fn binding() -> AssignedRemoteBinding {
    AssignedRemoteBinding {
        line: AssignedIdentity::from_text("selected-line"),
        local_host: requirements(&[]).host,
        local_boot: requirements(&[]).boot,
        peer_host: AssignedIdentity::from_text("recorder-host"),
        peer_boot: AssignedIdentity::from_text("recorder-boot"),
    }
}
fn fore(endpoint: u16, direction: AssignedForeDirection) -> AssignedForeRequirement {
    AssignedForeRequirement::new(
        endpoint,
        endpoint + 4,
        direction,
        AssignedIdentity::from_text("checked-fore"),
        [3; 32],
    )
}
fn line(endpoint: u16) -> AssignedLineForeRequirement {
    AssignedLineForeRequirement::new(
        fore(endpoint, AssignedForeDirection::Output),
        binding(),
        AssignedIdentity::from_text("session"),
        AssignedIdentity::from_text("noise-profile"),
        [7; 32],
    )
}
fn envelope(payloads: &[Vec<u8>], schema: u16) -> Vec<u8> {
    let mut bytes = vec![0; ASSIGNED_PLAN_HEADER_BYTES];
    bytes[..8].copy_from_slice(&assigned_plan_magic());
    bytes[8..10].copy_from_slice(&schema.to_le_bytes());
    bytes[12..14].copy_from_slice(&128u16.to_le_bytes());
    bytes[48..64].copy_from_slice(&requirements(&[]).host.0);
    bytes[64..80].copy_from_slice(&requirements(&[]).boot.0);
    bytes[89] = payloads.len() as u8;
    for payload in payloads {
        bytes.push(ASSIGNED_REMOTE_ENDPOINT);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    let len = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&len.to_le_bytes());
    let digest = assigned_plan_payload_digest(&bytes[125..]);
    bytes[93..125].copy_from_slice(&digest);
    bytes
}
fn maxima() -> AssignedPlanMaxima {
    let mut maxima = AssignedPlanMaxima::TINY_HOST;
    // This mixed fixture explicitly funds three records. Production profile
    // ceilings and the original TINY_HOST inventory remain unchanged.
    maxima.counts[9] = 16;
    maxima
}
fn decode(
    bytes: &[u8],
    local: &[AssignedForeRequirement],
    lines: &[AssignedLineForeRequirement],
    bindings: &[AssignedRemoteBinding],
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    let (result, allocations) = crate::record_view_tests::allocation_probe::observe(|| {
        decode_assigned_plan_with_line_fore(
            bytes,
            maxima(),
            requirements(bindings),
            computations(),
            AssignedLineForeProfile { local, lines },
        )
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    result
}
#[test]
fn exact_mixed_fore_and_shared_line_are_opt_in_and_allocation_free() {
    let local = [fore(0, AssignedForeDirection::Input)];
    let lines = [line(1), line(2)];
    let payloads = [
        assigned_line_fore_local_payload(&local[0]).to_vec(),
        lines[1].payload().to_vec(),
        lines[0].payload().to_vec(),
    ];
    let bytes = envelope(&payloads, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let view = decode(&bytes, &local, &lines, &[binding()]).unwrap();
    assert_eq!(
        decode_assigned_plan_with_line_fore(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            requirements(&[binding()]),
            computations(),
            AssignedLineForeProfile {
                local: &local,
                lines: &lines
            }
        ),
        Err(AssignedPlanRefusal::CountCapacityExceeded(10))
    );
    assert!(AssignedLocalProjectionTemplate::check(
        &bytes,
        &assigned_plan_payload_digest(&bytes[125..]),
        maxima()
    )
    .is_err());
    assert_eq!(view.counts[9], 3);
    assert_eq!(view.counts[12], 0); // Never repurpose Resource Cord inventory.
    assert_eq!(
        u16::from_le_bytes([bytes[8], bytes[9]]),
        ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA
    );
    assert_eq!(AssignedPlanRecords::new(&bytes).unwrap().count(), 3);
    assert!(decode_assigned_plan_with_fore(
        &bytes,
        AssignedPlanMaxima::TINY_HOST,
        requirements(&[]),
        computations(),
        &local
    )
    .is_err());
    assert!(decode_assigned_plan_with_computations(
        &bytes,
        AssignedPlanMaxima::TINY_HOST,
        requirements(&[binding()]),
        computations()
    )
    .is_err());
    for schema in 4..=7 {
        assert!(decode(&envelope(&payloads, schema), &local, &lines, &[binding()]).is_err());
    }
}
#[test]
fn every_line_pin_is_exact_including_direction_bounds_session_and_peer() {
    let expected = [line(1)];
    // Reseal the record digest after each mutation: rejection must come from
    // trusted pins, rather than treating received bytes as their own authority.
    for offset in 0..ASSIGNED_LINE_FORE_BYTES {
        let mut payload = expected[0].payload().to_vec();
        payload[offset] ^= 1;
        assert!(
            decode(&envelope(&[payload], 8), &[], &expected, &[binding()]).is_err(),
            "changed Line/Fore commitment byte {offset}"
        );
    }
    let bytes = envelope(&[expected[0].payload().to_vec()], 8);
    for offset in 48..80 {
        let mut stale = bytes.clone();
        stale[offset] ^= 1;
        assert!(decode(&stale, &[], &expected, &[binding()]).is_err());
    }
}
#[test]
fn missing_duplicate_foreign_and_truncated_records_refuse() {
    let lines = [line(1), line(2)];
    assert!(decode(
        &envelope(&[lines[0].payload().to_vec()], 8),
        &[],
        &lines,
        &[binding()]
    )
    .is_err());
    assert!(decode(
        &envelope(
            &[lines[0].payload().to_vec(), lines[0].payload().to_vec()],
            8
        ),
        &[],
        &lines,
        &[binding()]
    )
    .is_err());
    assert!(decode(
        &envelope(&[line(3).payload().to_vec()], 8),
        &[],
        &lines,
        &[binding()]
    )
    .is_err());
    for len in [0, 1, 53, 54, 197, 199, 250] {
        let mut payload = lines[0].payload().to_vec();
        payload.resize(len, 0);
        assert!(decode(&envelope(&[payload], 8), &[], &lines[..1], &[binding()]).is_err());
    }
    let local = fore(0, AssignedForeDirection::Input);
    let mut local_bytes = assigned_line_fore_local_payload(&local);
    local_bytes[22] ^= 1;
    assert!(decode(&envelope(&[local_bytes.to_vec()], 8), &[local], &[], &[]).is_err());
}
#[test]
fn expected_profile_must_match_original_remote_inventory_and_unique_fore() {
    let expected = [line(1)];
    let bytes = envelope(&[expected[0].payload().to_vec()], 8);
    assert!(decode(&bytes, &[], &expected, &[]).is_err());
    assert!(decode(&bytes, &[], &expected, &[binding(), binding()]).is_err());
    let mut unused = binding();
    unused.line = AssignedIdentity::from_text("unused-line");
    assert!(decode(&bytes, &[], &expected, &[binding(), unused]).is_err());
    let too_many: Vec<_> = (0..17).map(line).collect();
    assert!(decode(&bytes, &[], &too_many, &[binding()]).is_err());
    let mut foreign = binding();
    foreign.peer_boot = AssignedIdentity::from_text("other-boot");
    assert!(decode(&bytes, &[], &expected, &[foreign]).is_err());
    let mut stale = binding();
    stale.local_boot = AssignedIdentity::from_text("stale-local");
    let stale_line = AssignedLineForeRequirement::new(
        fore(1, AssignedForeDirection::Output),
        stale,
        AssignedIdentity([1; 16]),
        AssignedIdentity([2; 16]),
        [3; 32],
    );
    assert!(decode(
        &envelope(&[stale_line.payload().to_vec()], 8),
        &[],
        &[stale_line],
        &[stale]
    )
    .is_err());
    assert!(decode(&bytes, &[], &[expected[0], expected[0]], &[binding()]).is_err());
    assert!(decode(
        &bytes,
        &[fore(1, AssignedForeDirection::Input)],
        &expected,
        &[binding()]
    )
    .is_err());
    let same_cord = AssignedForeRequirement::new(
        7,
        5,
        AssignedForeDirection::Input,
        AssignedIdentity([1; 16]),
        [4; 32],
    );
    assert!(decode(&bytes, &[same_cord], &expected, &[binding()]).is_err());
}
