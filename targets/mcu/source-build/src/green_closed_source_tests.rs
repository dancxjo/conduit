//! Closed startup/bus sampling core, deliberately not an application claim.
//! Terminal observation/failure Fores remain visible until standard Mask routing
//! is integrated. No device, grant, Started Play or firmware is fabricated here.
use super::super::{contract, InstalledValueBindings, InstalledValueInput};
use crate::{composite::ValueReference, linked_source::LinkedSourceImage, workset};
use conduit_core as c;
use sha2::{Digest, Sha256};

fn settings(source: &conduit_plot::CheckedSyntaxDocument) -> InstalledValueInput {
    let native = source
        .native_types
        .iter()
        .find(|ty| ty.name == "BmeSamplingBegin")
        .unwrap();
    fn value(schema: &c::StructuredInfoType, name: &str) -> c::StructuredInfoValue {
        match schema.shape() {
            c::StructuredInfoTypeShape::Record { fields, .. } => c::StructuredInfoValue::record(
                schema.clone(),
                fields
                    .iter()
                    .map(|field| {
                        c::StructuredFieldValue::new(
                            field.name(),
                            value(field.value_type(), field.name()),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
            c::StructuredInfoTypeShape::Leaf(_) => c::StructuredInfoValue::leaf(
                schema.clone(),
                match name {
                    "address" => vec![118],
                    "count" => 2_u64.to_le_bytes().to_vec(),
                    "period_ms" => 1000_u64.to_le_bytes().to_vec(),
                    _ => panic!("unreviewed installed field {name}"),
                },
            )
            .unwrap(),
            _ => panic!("unreviewed installed schema"),
        }
    }
    InstalledValueInput {
        key: "environment-sampling".into(),
        type_name: "BmeSamplingBegin".into(),
        maximum_bytes: 359,
        image: LinkedSourceImage::prepare(native, &value(&native.value_type, "")).unwrap(),
    }
}

#[test]
fn closed_sampling_core_accounts_original_owner_work_before_mask() {
    let (library, selected) = crate::bme_sampling_build_tests::selected_source();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    crate::protocol_catalog::install(&mut startup, &mut profiles).unwrap();
    let types = conduit_plot::prepare_source_types(
        &conduit_plot::parse_syntax_document(&library),
        &startup,
    )
    .unwrap();
    let schema = types.named_type("BmeSamplingBegin").unwrap();
    let kind = conduit_semantic_catalog::installed_value_once_semantic_contract(
        &contract(schema, 359).unwrap(),
        schema,
    )
    .unwrap();
    let source = format!(
        r#"with {} as SamplingConfiguration
{library}
plot closed-sampling-core (
 captured: BmeProtocolState...| >>
 observation: BmeObservationResult...| >>
 refused: BmeSamplingFailure...| >>
 clock_failed: ClockResult...| >>
) {{
 configuration: SamplingConfiguration(key = "environment-sampling")
 sampler: sampled-environment
 bus: machine/i2c/transact
 configuration.value >> sampler.begin
 sampler.request >> bus.request
 bus.result >> sampler.result
 sampler.captured >> captured
 sampler.observation >> observation
 sampler.refused >> refused
 sampler.clock_failed >> clock_failed
}}
"#,
        kind.kind_id.as_str()
    );
    let sha = format!("{:x}", Sha256::digest(source.as_bytes()));
    let set = workset::compile_specialized_with_installed(
        source.as_bytes(),
        &sha,
        &["closed-sampling-core".into()],
        None,
        &selected,
        &[ValueReference {
            type_name: "BmeSamplingBegin",
            maximum_bytes: 359,
        }],
        |checked| {
            InstalledValueBindings::prepare(
                "fixture/green-sampling-core",
                checked,
                vec![settings(checked)],
            )
        },
    )
    .unwrap();
    let entry = &set.entries[0];
    assert!(
        entry.plot.input_bindings.is_empty(),
        "startup and bus feedback must be closed"
    );
    let material = crate::software::compile_with_installed(
        &entry.plot.expanded,
        Some(&set.checked_source),
        24 * 1024,
        &set.installed_values,
    )
    .unwrap();
    assert_eq!(
        material
            .nodes
            .iter()
            .filter(|n| matches!(n.role, crate::software::SoftwareRole::Installed { .. }))
            .count(),
        1
    );
    assert_eq!(
        entry
            .plot
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "machine/i2c/transact")
            .count(),
        1
    );
    assert_eq!(
        entry
            .plot
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "machine/clock/at")
            .count(),
        1
    );
    let mut bounds = crate::resident_expression_bounds(4997);
    bounds.native.canonical_bytes = 8192;
    // No aggregate pool is supplied and none of these diagnostic owners Starts.
    // Diagnostic compiler envelope only: collect the complete demand even when
    // one original owner already exceeds the target pool. This is never passed
    // to calibration/admission, and cannot qualify a Green artifact.
    let mut diagnostic_bounds = bounds;
    diagnostic_bounds.private_work_units = u16::MAX - 3;
    let inventory = set.prepare_selected_residents(diagnostic_bounds).unwrap();
    let total_work: u32 = inventory
        .iter()
        .map(|r| u32::from(r.computation_work()))
        .sum();
    // The original Owned Back has canonical output and scratch [u8; B].
    // Their payload bound is independent of host pointer width and excludes all
    // native metadata, software, Cord store, kernel and allocator overhead.
    let owned_payload_bytes: u32 = inventory
        .iter()
        .map(|owner| {
            2 * owner
                .offer_description()
                .semantic_contract
                .value_contracts()
                .iter()
                .find_map(|v| {
                    matches!(v.location, c::FrontValueLocation::Output(_))
                        .then_some(v.contract.maximum_bytes)
                })
                .unwrap()
        })
        .sum();
    println!("GREEN_CLOSED_PAYLOAD architecture_independent_owned_output_plus_scratch={owned_payload_bytes} original_entire_heap=49152");
    assert_eq!(owned_payload_bytes, 208884);
    assert!(owned_payload_bytes > 49152);
    let mut generated = String::new();
    let mut initializers = String::new();
    let mut inline = String::new();
    for (i, owner) in inventory.iter().enumerate() {
        use std::fmt::Write;
        writeln!(generated, "mod owner_{i}{{{}}}", owner.source()).unwrap();
        write!(initializers, "owner_{i}::prepare_back().unwrap(),").unwrap();
        write!(inline, "+core::mem::size_of::<owner_{i}::Back>()").unwrap();
    }
    println!(
        "GREEN_CLOSED_INVENTORY residents={} exact_private_work={total_work} original_pool=5000",
        inventory.len()
    );
    assert!(total_work > 5000);
    let largest = inventory
        .iter()
        .max_by_key(|r| r.computation_work())
        .unwrap();
    println!(
        "GREEN_CLOSED_LARGEST exact_private_work={} kind={}",
        largest.computation_work(),
        largest.kind_id().as_str()
    );
    assert!(largest.computation_work() > 4997);
    let probe = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../architecture/plot/tests/common/allocation_probe.rs"
    ))
    .unwrap();
    let exercise = format!(
        r#"
mod allocation {{
{probe}
pub fn live() -> usize {{ OBSERVATION.with(|o| o.get().live_bytes) }}
}}
#[global_allocator] static ALLOCATOR: allocation::Allocator = allocation::Allocator;
fn main() {{
 let (owners, report) = allocation::observe(|| ({initializers}));
 let retained_heap = allocation::live();
 let inline_bytes = 0usize {inline};
 println!("GREEN_CLOSED_RESIDENT_STORAGE target=host-usize owned_residents={} retained_heap={{retained_heap}} inline_bytes={{inline_bytes}} preparation_peak_heap={{}} runtime_execution=not-started", report.peak_bytes);
 assert!(retained_heap + inline_bytes > 49152);
 drop(owners);
}}
"#,
        inventory.len()
    );
    crate::compiled_rust::verify_with_libraries(
        &generated,
        &exercise,
        "green-closed-sampling-storage",
        &[
            "conduit_core",
            "conduit_plot",
            "conduit_kernel",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
    let result = set.prepare_selected_residents_with_exact_work(
        bounds,
        conduit_embedded_build::ResidentExecutionStorage::Owned,
        5000,
    );
    let refusal = result
        .err()
        .expect("original Green pool must refuse before Mask integration");
    println!(
        "GREEN_CLOSED_CORE source_sha256={sha} nodes={} cords={} software={} refusal={refusal}",
        entry.nodes,
        entry.cords,
        material.nodes.len()
    );
    assert!(
        refusal.contains("private work requires"),
        "unexpected preparation gate: {refusal}"
    );
}
