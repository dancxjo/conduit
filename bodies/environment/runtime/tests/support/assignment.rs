//! Real Source computations cross bounded allocation-free assignment admission.
use conduit_core::*;
use conduit_embedded_build::{
    assigned_computation_requirements, encode_assigned_plan, GeneratedEmbeddedPlan,
};

pub fn prepare<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    image: &GeneratedEmbeddedPlan,
    check_refusals: bool,
) -> conduit_environment_runtime::assigned::AssignedKernelTables<N, C, P, R, T, H> {
    prepare_from_resident::<N, C, P, R, T, H>(image, image, check_refusals)
}
pub fn prepare_from_resident<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    image: &GeneratedEmbeddedPlan,
    resident: &GeneratedEmbeddedPlan,
    check_refusals: bool,
) -> conduit_environment_runtime::assigned::AssignedKernelTables<N, C, P, R, T, H> {
    let maxima = AssignedPlanMaxima {
        encoded_bytes: 48 * 1024,
        runtime_state_bytes: 16 * 1024,
        counts: [16, 32, 8, 16, 16, 16, 8, 1, 128, 0, 32, 32, 0],
    };
    let encoded = conduit_embedded_build::encode_assigned_native_plan(image, maxima).unwrap();
    let legacy = encode_assigned_plan(image, maxima).unwrap();
    let inventory = assigned_computation_requirements(resident).unwrap();
    assert_eq!(inventory.len(), 7);
    let calls: Vec<_> = image
        .host_calls
        .iter()
        .map(|c| AssignedIdentity::from_text(&c.contract_id))
        .collect();
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
    let required = AssignedPlanRequirements {
        host: AssignedIdentity::from_text(&image.host_id),
        boot: AssignedIdentity::from_text(&image.boot_id),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let profile = AssignedComputationProfile {
        maximum_text_bytes: 32768,
        maximum_total_text_bytes: 45000,
        requirements: &inventory,
    };
    assert_eq!(
        decode_assigned_plan(&legacy, maxima, required),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
    assert_eq!(
        decode_assigned_plan(&encoded, maxima, required),
        Err(AssignedPlanRefusal::WrongSchema)
    );
    let (view, allocation) = super::allocation_probe::observe(|| {
        decode_assigned_plan_with_computations(&encoded, maxima, required, profile).unwrap()
    });
    assert_eq!(view.plan, AssignedIdentity::from_text(&image.plan_id));
    assert_eq!(
        view.fragment,
        AssignedIdentity::from_text(&image.fragment_id)
    );
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let structural =
        conduit_embedded_build::assigned_native_layout_requirements(resident, maxima).unwrap();
    let resident_bytes =
        conduit_embedded_build::encode_assigned_native_plan(resident, maxima).unwrap();
    let mut resident_counts = [0; ASSIGNED_PLAN_COUNT_KINDS];
    resident_counts.copy_from_slice(&resident_bytes[80..80 + ASSIGNED_PLAN_COUNT_KINDS]);
    let layout = conduit_environment_runtime::assigned::ResidentLayout {
        records: &structural,
        counts: resident_counts,
    };
    let (tables, allocated) = super::allocation_probe::observe(|| {
        conduit_environment_runtime::assigned::AssignedKernelTables::<N, C, P, R, T, H>::prepare(
            &encoded, maxima, required, profile, &layout,
        )
        .unwrap()
    });
    assert_eq!((allocated.allocations, allocated.reallocations), (0, 0));
    assert_eq!(tables.identity, view);
    if !check_refusals {
        return tables;
    }
    assert!(matches!(
        conduit_environment_runtime::assigned::AssignedKernelTables::<N, C, P, R, T, H>::prepare(
            &legacy, maxima, required, profile, &layout
        ),
        Err(conduit_environment_runtime::assigned::AssignedKernelRefusal::Unsupported)
    ));
    // Representation fixture only: prove schema 5 does not silently lose a
    // nondefault pressure policy or single-value envelope, independently of Play.
    let mut alternate = image.clone();
    alternate.cords[0].maximum_value_bytes = alternate.cords[0].byte_capacity.saturating_sub(1);
    alternate.cords[0].pressure_policy =
        conduit_kernel::scheduler::AssignedPressurePolicy::CoalesceLatest;
    let alternate_tables = prepare_from_resident::<N, C, P, R, T, H>(&alternate, &alternate, false);
    assert_eq!(
        alternate_tables.cords[0].maximum_value_bytes,
        alternate.cords[0].maximum_value_bytes
    );
    assert_eq!(
        alternate_tables.cords[0].pressure_policy,
        alternate.cords[0].pressure_policy
    );
    eprintln!("environment_kernel_schema={} encoded_bytes={} declared_runtime_projection_bytes={} structural_records={}", ASSIGNED_KERNEL_PLAN_SCHEMA, encoded.len(), view.runtime_state_bytes, structural.len());
    let total: u32 = inventory.iter().map(|r| u32::from(r.text_bytes)).sum();
    let largest = inventory.iter().map(|r| r.text_bytes).max().unwrap();
    for constrained in [
        AssignedComputationProfile {
            maximum_text_bytes: largest - 1,
            ..profile
        },
        AssignedComputationProfile {
            maximum_total_text_bytes: total - 1,
            ..profile
        },
    ] {
        assert_eq!(
            decode_assigned_plan_with_computations(&encoded, maxima, required, constrained),
            Err(AssignedPlanRefusal::ComputationCapacityExceeded)
        );
    }
    for changed in 0..3 {
        let mut wrong = inventory.clone();
        match changed {
            0 => wrong[0].text_digest[0] ^= 1,
            1 => wrong[0].node = u16::MAX,
            _ => wrong[0].text_bytes -= 2,
        }
        assert_eq!(
            decode_assigned_plan_with_computations(
                &encoded,
                maxima,
                required,
                AssignedComputationProfile {
                    requirements: &wrong,
                    ..profile
                }
            ),
            Err(AssignedPlanRefusal::UnknownComputation)
        );
    }
    let mut duplicate = inventory.clone();
    duplicate.push(inventory[0]);
    assert_eq!(
        decode_assigned_plan_with_computations(
            &encoded,
            maxima,
            required,
            AssignedComputationProfile {
                requirements: &duplicate,
                ..profile
            }
        ),
        Err(AssignedPlanRefusal::MalformedRecord)
    );
    let mut missing = inventory.clone();
    let mut extra = inventory[0];
    extra.node = 65535;
    missing.push(extra);
    assert_eq!(
        decode_assigned_plan_with_computations(
            &encoded,
            maxima,
            required,
            AssignedComputationProfile {
                requirements: &missing,
                ..profile
            }
        ),
        Err(AssignedPlanRefusal::MissingComputation)
    );
    let mut stale = required;
    stale.boot = AssignedIdentity::from_text("stale/boot");
    assert_eq!(
        decode_assigned_plan_with_computations(&encoded, maxima, stale, profile),
        Err(AssignedPlanRefusal::WrongBoot)
    );
    let mut corrupt = encoded.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert_eq!(
        decode_assigned_plan_with_computations(&corrupt, maxima, required, profile),
        Err(AssignedPlanRefusal::DigestMismatch)
    );
    // Even a re-digested assignment cannot substitute program text for the
    // resident trusted inventory. The assignment digest is integrity, not authority.
    let mut substituted = encoded.clone();
    let mut cursor = ASSIGNED_PLAN_HEADER_BYTES;
    while substituted[cursor] != ASSIGNED_CONFIGURATION {
        cursor += 3 + usize::from(u16::from_le_bytes([
            substituted[cursor + 1],
            substituted[cursor + 2],
        ]));
    }
    let index = cursor + 3 + 19;
    substituted[index] = if substituted[index] == b'0' {
        b'1'
    } else {
        b'0'
    };
    let digest = assigned_plan_payload_digest(&substituted[ASSIGNED_PLAN_HEADER_BYTES..]);
    let digest_start = 80 + ASSIGNED_PLAN_COUNT_KINDS;
    substituted[digest_start..digest_start + 32].copy_from_slice(&digest);
    assert_eq!(
        decode_assigned_plan_with_computations(&substituted, maxima, required, profile),
        Err(AssignedPlanRefusal::UnknownComputation)
    );
    let mut changed_table = encoded;
    // Node zero's fuel is valid in the format but must match the resident image.
    let fuel = ASSIGNED_PLAN_HEADER_BYTES + 3 + 2;
    changed_table[fuel] ^= 1;
    let digest = assigned_plan_payload_digest(&changed_table[ASSIGNED_PLAN_HEADER_BYTES..]);
    changed_table[digest_start..digest_start + 32].copy_from_slice(&digest);
    assert!(
        decode_assigned_plan_with_computations(&changed_table, maxima, required, profile).is_ok()
    );
    assert!(matches!(
        conduit_environment_runtime::assigned::AssignedKernelTables::<N, C, P, R, T, H>::prepare(
            &changed_table,
            maxima,
            required,
            profile,
            &layout
        ),
        Err(conduit_environment_runtime::assigned::AssignedKernelRefusal::Layout)
    ));
    tables
}

pub struct NativeProjection {
    pub bytes: Vec<u8>,
    pub computations: Vec<AssignedComputationRequirement>,
    pub calls: Vec<AssignedIdentity>,
    pub resources: Vec<u16>,
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
}
impl NativeProjection {
    pub fn prepare(image: &GeneratedEmbeddedPlan) -> Self {
        Self {
            bytes: conduit_embedded_build::encode_assigned_native_plan(image, Self::maxima())
                .unwrap(),
            computations: assigned_computation_requirements(image).unwrap(),
            calls: image
                .host_calls
                .iter()
                .map(|c| AssignedIdentity::from_text(&c.contract_id))
                .collect(),
            resources: image.resources.iter().map(|r| r.resource).collect(),
            host: AssignedIdentity::from_text(&image.host_id),
            boot: AssignedIdentity::from_text(&image.boot_id),
        }
    }
    pub fn maxima() -> AssignedPlanMaxima {
        AssignedPlanMaxima {
            encoded_bytes: 48 * 1024,
            runtime_state_bytes: 16 * 1024,
            counts: [16, 32, 8, 16, 16, 16, 8, 1, 128, 0, 32, 32, 0],
        }
    }
    pub fn required(&self) -> AssignedPlanRequirements<'_> {
        AssignedPlanRequirements {
            host: self.host,
            boot: self.boot,
            host_calls: &self.calls,
            resources: &self.resources,
            remote_bindings: &[],
        }
    }
    pub fn profile(&self) -> AssignedComputationProfile<'_> {
        AssignedComputationProfile {
            maximum_text_bytes: 32768,
            maximum_total_text_bytes: 45000,
            requirements: &self.computations,
        }
    }
}
