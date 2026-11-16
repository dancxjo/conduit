//! Host-only geometry/schema generation. The process exits before the independent
//! store measurement runs; no authoring AST or dynamically leaked schema survives.
use conduit_mcu_source_build as source_build;
#[allow(dead_code)]
#[path = "../fixtures/environmental_exchange_source.rs"]
mod environmental_source;
mod fabrication_inputs;
#[allow(dead_code)]
#[path = "../../../../rp2040/firmware/host/i2c_workset_build.rs"]
mod i2c_workset_build;
use std::{fmt::Write, path::PathBuf};
fn main() {
    let root = PathBuf::from(std::env::args_os().nth(1).expect("explicit repository"));
    let out = PathBuf::from(std::env::args_os().nth(2).expect("explicit output"));
    let profile =
        root.join("targets/rp2040/profiles/pico-w-house-green-source-slots-candidate.host.conduit");
    let input = fabrication_inputs::prepare(&out, &profile);
    let mut entries = i2c_workset_build::inspect_schema(
        &input.checked,
        &input.catalog,
        &input.image.manifest.image_id,
        &input.identity,
        &out,
    )
    .unwrap()
    .unwrap();
    assert_eq!(entries.len(), 1);
    let entry = entries.pop().unwrap();
    let kernel = entry.lowering.generated.kernel();
    assert_eq!(
        (kernel.cord_value_slots, kernel.cord_value_bytes),
        (97, 164818)
    );
    let mut profiles = entry
        .value_profiles_rust
        .expect("explicit existing schema profile");
    let results: Vec<_> = kernel
        .host_calls
        .iter()
        .flat_map(|call| {
            std::iter::repeat_n(
                call.maximum_output_bytes,
                usize::from(call.maximum_in_flight),
            )
        })
        .filter(|bytes| *bytes != 0)
        .collect();
    writeln!(
        profiles,
        "pub const NATIVE_RESULT_CAPACITIES:&[u32]=&{results:?};"
    )
    .unwrap();
    writeln!(
        profiles,
        "pub const ASSIGNED_CORD_SLOTS:u16={};",
        kernel.cord_value_slots
    )
    .unwrap();
    writeln!(
        profiles,
        "pub const ASSIGNED_CORD_BYTES:u32={};",
        kernel.cord_value_bytes
    )
    .unwrap();
    std::fs::write(out.join("schema_profiles.rs"), profiles).unwrap();
    let geometry = serde_json::json!({
        "scope":"inert open-Fore protocol service only; no autonomous sampling or Face/Mask",
        "source":entry.lowering.plan.source_document_id, "entry":entry.name,
        "configuration":input.checked.configuration_id(), "image":input.image.manifest.image_id,
        "logical_cord_slots":kernel.cord_value_slots, "logical_cord_bytes":kernel.cord_value_bytes,
        "nodes":kernel.nodes.len(), "cords":kernel.cords.len(),
        "route_targets":kernel.route_targets.len(), "native_calls":kernel.host_calls.len(),
        "sign_items":kernel.sign_items, "sign_bytes":kernel.sign_bytes,
        "legacy_canonical_runtime_state_estimate":conduit_embedded_build::assigned_runtime_state_bytes(kernel).unwrap(),
        "host_heap_unchanged":input.checked.configuration().limits.heap_arena_bytes,
        "host_buffered_bytes_unchanged":input.checked.configuration().limits.buffered_bytes,
        "host_assigned_runtime_unchanged":input.checked.configuration().preparation.as_ref().unwrap().maximum_runtime_state_bytes,
        "metadata_bounds":"existing HOST_TOOLING only; not a target resource grant",
        "assignment_encoded":false, "runtime_admission":false, "hardware_effects":false,
    });
    std::fs::write(
        out.join("geometry.json"),
        serde_json::to_vec_pretty(&geometry).unwrap(),
    )
    .unwrap();
    println!("{geometry}");
}
