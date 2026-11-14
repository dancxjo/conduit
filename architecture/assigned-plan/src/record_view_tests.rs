//! Canonical slice/view equivalence and bounded no-allocation admission.
use crate::*;
use std::{vec, vec::Vec};
pub(super) mod allocation_probe {
    use std::thread_local;
    include!("../../plot/tests/common/allocation_probe.rs");
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

type EncodedRecords = Vec<(u8, Vec<u8>)>;
fn fixture(
    schema: u16,
) -> (
    [u8; ASSIGNED_PLAN_HEADER_BYTES],
    EncodedRecords,
    AssignedComputationRequirement,
) {
    let call = AssignedIdentity::from_text("fixture/call");
    let program = AssignedComputationRequirement::from_text(0, "program", "0102").unwrap();
    let mut config = vec![0; 19];
    config[2..18].copy_from_slice(&program.key.0);
    config[18] = 3;
    config.extend_from_slice(b"0102");
    let mut operation = vec![
        0;
        if schema == ASSIGNED_NATIVE_PLAN_SCHEMA {
            47
        } else {
            46
        }
    ];
    operation[4..20].copy_from_slice(&call.0);
    let records = vec![
        (ASSIGNED_NODE, vec![0; 52]),
        (ASSIGNED_PORT, vec![0; 37]),
        (ASSIGNED_CONFIGURATION, config),
        (ASSIGNED_HOST_CALL, operation),
        (ASSIGNED_RESOURCE, vec![0; 8]),
    ];
    let mut header = [0; ASSIGNED_PLAN_HEADER_BYTES];
    header[..8].copy_from_slice(&assigned_plan_magic());
    header[8..10].copy_from_slice(&schema.to_le_bytes());
    header[12..14].copy_from_slice(&128u16.to_le_bytes());
    header[48..64].copy_from_slice(&AssignedIdentity::from_text("fixture/host").0);
    header[64..80].copy_from_slice(&AssignedIdentity::from_text("fixture/boot").0);
    for (tag, _) in &records {
        header[80 + usize::from(*tag - 1)] += 1;
    }
    seal(&mut header, &records);
    (header, records, program)
}
fn seal(header: &mut [u8; ASSIGNED_PLAN_HEADER_BYTES], records: &[(u8, Vec<u8>)]) {
    let payload = framed(records);
    let length = (ASSIGNED_PLAN_HEADER_BYTES + payload.len()) as u16;
    header[10..12].copy_from_slice(&length.to_le_bytes());
    header[93..125].copy_from_slice(&assigned_plan_payload_digest(&payload));
}
fn framed(records: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (tag, payload) in records {
        bytes.push(*tag);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    bytes
}
fn compare(
    header: &[u8; ASSIGNED_PLAN_HEADER_BYTES],
    data: &[(u8, Vec<u8>)],
    program: AssignedComputationRequirement,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    compare_with_maxima(header, data, program, AssignedPlanMaxima::TINY_HOST)
}
fn compare_with_maxima(
    header: &[u8; ASSIGNED_PLAN_HEADER_BYTES],
    data: &[(u8, Vec<u8>)],
    program: AssignedComputationRequirement,
    maxima: AssignedPlanMaxima,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    let records: Vec<_> = data
        .iter()
        .map(|(tag, payload)| AssignedRecord { tag: *tag, payload })
        .collect();
    let mut bytes = header.to_vec();
    bytes.extend(framed(data));
    let calls = [AssignedIdentity::from_text("fixture/call")];
    let resources = [0];
    let computations = [program];
    let required = AssignedPlanRequirements {
        host: AssignedIdentity::from_text("fixture/host"),
        boot: AssignedIdentity::from_text("fixture/boot"),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let profile = AssignedComputationProfile {
        maximum_text_bytes: 16,
        maximum_total_text_bytes: 16,
        requirements: &computations,
    };
    let (actual, memory) = allocation_probe::observe(|| {
        decode_assigned_records_with_computations(header, &records, maxima, required, profile)
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        actual,
        decode_assigned_plan_with_computations(&bytes, maxima, required, profile)
    );
    actual
}
#[test]
fn borrowed_records_preserve_all_schemas_and_single_mutation_refusals() {
    for schema in [
        ASSIGNED_PLAN_SCHEMA,
        ASSIGNED_KERNEL_PLAN_SCHEMA,
        ASSIGNED_NATIVE_PLAN_SCHEMA,
    ] {
        let (header, records, program) = fixture(schema);
        assert!(compare(&header, &records, program).is_ok());
        for index in 0..header.len() {
            let mut altered = header;
            altered[index] ^= 1;
            let _ = compare(&altered, &records, program);
        }
        for index in 0..records.len() {
            for offset in 0..records[index].1.len() {
                let mut altered = records.clone();
                altered[index].1[offset] ^= 1;
                assert_eq!(
                    compare(&header, &altered, program),
                    Err(AssignedPlanRefusal::DigestMismatch)
                );
                let mut resealed = header;
                seal(&mut resealed, &altered);
                let _ = compare(&resealed, &altered, program);
            }
        }
    }
}
#[test]
fn incremental_hash_matches_legacy_at_every_split_and_padding_boundary() {
    for length in [0, 1, 55, 56, 63, 64, 65, 127, 128, 129, 1024] {
        let bytes: Vec<_> = (0..length).map(|i| (i % 251) as u8).collect();
        for split in 0..=bytes.len() {
            let mut state = sha256::State::new();
            state.update(&bytes[..split]);
            state.update(&[]);
            state.update(&bytes[split..]);
            assert_eq!(state.finish(), sha256::digest(&bytes));
        }
    }
}

#[test]
fn record_view_refuses_unrepresentable_lengths_and_inventory_before_growth() {
    let (header, data, program) = fixture(ASSIGNED_NATIVE_PLAN_SCHEMA);
    let calls = [AssignedIdentity::from_text("fixture/call")];
    let resources = [0];
    let computations = [program];
    let required = AssignedPlanRequirements {
        host: AssignedIdentity::from_text("fixture/host"),
        boot: AssignedIdentity::from_text("fixture/boot"),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let profile = AssignedComputationProfile {
        maximum_text_bytes: 16,
        maximum_total_text_bytes: 16,
        requirements: &computations,
    };
    let check = |head: &[u8], records: &[AssignedRecord<'_>], maxima| {
        let (result, memory) = allocation_probe::observe(|| {
            decode_assigned_records_with_computations(head, records, maxima, required, profile)
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        result
    };
    let records: Vec<_> = data
        .iter()
        .map(|(tag, payload)| AssignedRecord { tag: *tag, payload })
        .collect();
    assert_eq!(
        check(&header[..124], &records, AssignedPlanMaxima::TINY_HOST),
        Err(AssignedPlanRefusal::WrongLength)
    );
    let mut insufficient = AssignedPlanMaxima::TINY_HOST;
    insufficient.encoded_bytes = 125;
    assert_eq!(
        check(&header, &records, insufficient),
        Err(AssignedPlanRefusal::EncodedCapacityExceeded)
    );
    let oversized = vec![0; usize::from(u16::MAX) + 1];
    assert_eq!(
        check(
            &header,
            &[AssignedRecord {
                tag: ASSIGNED_NODE,
                payload: &oversized
            }],
            AssignedPlanMaxima::TINY_HOST
        ),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
    let too_many = vec![
        AssignedRecord {
            tag: ASSIGNED_NODE,
            payload: &[]
        };
        ASSIGNED_PLAN_COUNT_KINDS * usize::from(u8::MAX) + 1
    ];
    assert_eq!(
        check(&header, &too_many, AssignedPlanMaxima::TINY_HOST),
        Err(AssignedPlanRefusal::ExtraRecords)
    );
}

#[test]
fn resident_fingerprints_require_exact_trusted_inventory_and_do_not_allocate() {
    for schema in [
        ASSIGNED_PLAN_SCHEMA,
        ASSIGNED_KERNEL_PLAN_SCHEMA,
        ASSIGNED_NATIVE_PLAN_SCHEMA,
    ] {
        let (mut header, mut records, pin) = fixture(schema);
        records[2].1.truncate(19);
        records[2].1[18] = 4;
        records[2]
            .1
            .extend_from_slice(&pin.text_bytes.to_le_bytes());
        records[2].1.extend_from_slice(&pin.text_digest);
        seal(&mut header, &records);
        assert!(compare(&header, &records, pin).is_ok());
        for mutation in 0..5 {
            let mut changed = records.clone();
            match mutation {
                0 => changed[2].1[54] ^= 1,
                1 => changed[2].1[19..23].copy_from_slice(&0u32.to_le_bytes()),
                2 => changed[2].1[2] ^= 1,
                3 => changed[2].1[19..23].copy_from_slice(&6u32.to_le_bytes()),
                _ => {
                    changed[2].1.pop();
                }
            }
            let mut sealed = header;
            seal(&mut sealed, &changed);
            assert!(compare(&sealed, &changed, pin).is_err());
        }
        let calls = [AssignedIdentity::from_text("fixture/call")];
        let resources = [0];
        let required = AssignedPlanRequirements {
            host: AssignedIdentity::from_text("fixture/host"),
            boot: AssignedIdentity::from_text("fixture/boot"),
            host_calls: &calls,
            resources: &resources,
            remote_bindings: &[],
        };
        let mut encoded = header.to_vec();
        encoded.extend(framed(&records));
        assert_eq!(
            decode_assigned_plan(&encoded, AssignedPlanMaxima::TINY_HOST, required),
            Err(if schema == ASSIGNED_PLAN_SCHEMA {
                AssignedPlanRefusal::UnknownComputation
            } else {
                AssignedPlanRefusal::WrongSchema
            })
        );
        let views: Vec<_> = records
            .iter()
            .map(|(tag, payload)| AssignedRecord { tag: *tag, payload })
            .collect();
        assert_eq!(
            decode_assigned_records_with_computations(
                &header,
                &views,
                AssignedPlanMaxima::TINY_HOST,
                required,
                AssignedComputationProfile {
                    maximum_text_bytes: 16,
                    maximum_total_text_bytes: 16,
                    requirements: &[]
                }
            ),
            Err(AssignedPlanRefusal::UnknownComputation)
        );
        let mut duplicate = records.clone();
        duplicate.push(records[2].clone());
        let mut sealed = header;
        sealed[82] += 1;
        seal(&mut sealed, &duplicate);
        assert!(compare(&sealed, &duplicate, pin).is_err());
        let mut missing = records.clone();
        missing.remove(2);
        let mut sealed = header;
        sealed[82] -= 1;
        seal(&mut sealed, &missing);
        assert_eq!(
            compare(&sealed, &missing, pin),
            Err(AssignedPlanRefusal::MissingComputation)
        );
    }
}
#[test]
fn large_logical_program_stays_within_the_original_encoded_envelope_and_budget() {
    let (mut header, mut data, _) = fixture(ASSIGNED_NATIVE_PLAN_SCHEMA);
    let pin =
        AssignedComputationRequirement::from_text(0, "program", &"01".repeat(40_000)).unwrap();
    assert_eq!(pin.text_bytes, 80_000);
    data[2].1.truncate(19);
    data[2].1[18] = 4;
    data[2].1.extend_from_slice(&pin.text_bytes.to_le_bytes());
    data[2].1.extend_from_slice(&pin.text_digest);
    seal(&mut header, &data);
    let records: Vec<_> = data
        .iter()
        .map(|(tag, payload)| AssignedRecord { tag: *tag, payload })
        .collect();
    let calls = [AssignedIdentity::from_text("fixture/call")];
    let resources = [0];
    let inventory = [pin];
    let required = AssignedPlanRequirements {
        host: AssignedIdentity::from_text("fixture/host"),
        boot: AssignedIdentity::from_text("fixture/boot"),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let profile = AssignedComputationProfile {
        maximum_text_bytes: 80_000,
        maximum_total_text_bytes: 80_000,
        requirements: &inventory,
    };
    let (result, cost) = allocation_probe::observe(|| {
        decode_assigned_records_with_computations(
            &header,
            &records,
            AssignedPlanMaxima::TINY_HOST,
            required,
            profile,
        )
    });
    assert!(result.is_ok());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    for profile in [
        AssignedComputationProfile {
            maximum_text_bytes: 79_998,
            ..profile
        },
        AssignedComputationProfile {
            maximum_total_text_bytes: 79_998,
            ..profile
        },
    ] {
        assert_eq!(
            decode_assigned_records_with_computations(
                &header,
                &records,
                AssignedPlanMaxima::TINY_HOST,
                required,
                profile
            ),
            Err(AssignedPlanRefusal::ComputationCapacityExceeded)
        );
    }
}

#[test]
fn wide_state_uses_reserved_header_word_without_growth_or_narrowing() {
    assert_eq!(ASSIGNED_PLAN_HEADER_BYTES, 125);
    for schema in [
        ASSIGNED_PLAN_SCHEMA,
        ASSIGNED_KERNEL_PLAN_SCHEMA,
        ASSIGNED_NATIVE_PLAN_SCHEMA,
    ] {
        let (mut header, records, program) = fixture(schema);
        for bytes in [65_535, 65_536, 218_989, u32::MAX] {
            header[12..16].copy_from_slice(&bytes.to_le_bytes());
            let mut maxima = AssignedPlanMaxima::TINY_HOST;
            maxima.runtime_state_bytes = bytes;
            let view = compare_with_maxima(&header, &records, program, maxima).unwrap();
            assert_eq!(view.runtime_state_bytes, bytes);
            maxima.runtime_state_bytes = bytes - 1;
            assert_eq!(
                compare_with_maxima(&header, &records, program, maxima),
                Err(AssignedPlanRefusal::RuntimeCapacityExceeded)
            );
        }
        header[12..16].copy_from_slice(&128u32.to_le_bytes());
        header[..8].copy_from_slice(b"CNDAP001");
        assert_eq!(
            compare(&header, &records, program),
            Err(AssignedPlanRefusal::WrongMagic)
        );
    }
    assert_eq!(
        AssignedPlanMaxima {
            encoded_bytes: u16::MAX,
            runtime_state_bytes: u32::MAX,
            counts: [0; ASSIGNED_PLAN_COUNT_KINDS]
        }
        .total_bytes(),
        u64::from(u32::MAX) + u64::from(u16::MAX)
    );
}
