//! Fresh ordinary Make/Source emit-only calibration. No target compilation,
//! runtime admission, grant issuance, provider initialization or physical IO.
use conduit_mcu_source_build as source_build;
#[allow(dead_code)]
#[path = "../fixtures/environmental_exchange_source.rs"]
mod environmental_source;
mod fabrication_inputs;
#[path = "../../../../rp2040/firmware/host/i2c_workset_build.rs"]
mod i2c_workset_build;
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("explicit repository"));
    let out = std::path::PathBuf::from(std::env::args_os().nth(2).expect("explicit output"));
    std::fs::create_dir_all(&out).unwrap();
    let profile = std::env::args_os()
        .nth(3)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            root.join(
                "targets/rp2040/profiles/pico-w-house-green-source-work-candidate.host.conduit",
            )
        });
    let fabrication_inputs::Inputs {
        checked,
        catalog,
        image,
        identity,
    } = fabrication_inputs::prepare(&out, &profile);
    assert_eq!(checked.configuration().limits.heap_arena_bytes, 49152);
    let work = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&checked).unwrap();
    assert_eq!(
        (work.capacity_units, work.bookkeeping_bytes),
        (15117, 16384)
    );
    let result = i2c_workset_build::prepare(
        &checked,
        &catalog,
        &image.manifest.image_id,
        &identity,
        &out,
    );
    let (prepared, refusal) = match result {
        Ok(Some(residents)) => (residents.len(), None),
        Ok(None) => panic!("explicit Source disappeared"),
        Err(error) => (0, Some(error)),
    };
    println!(
        "{}",
        serde_json::json!({
            "scope":"fresh checked Host IMAGE and open-Fore BME/HT protocol service calibration; emit-only",
            "configuration":checked.configuration_id(), "image":image.manifest.image_id,
            "resident_work_units":work.capacity_units, "bookkeeping_bytes":work.bookkeeping_bytes,
            "heap_bytes":checked.configuration().limits.heap_arena_bytes,
        "logical_queue_item_ceiling":checked.configuration().limits.queue_items,
        "assignment_byte_ceiling":checked.configuration().preparation.as_ref().unwrap().maximum_assignment_bytes,
        "runtime_state_byte_ceiling":checked.configuration().preparation.as_ref().unwrap().maximum_runtime_state_bytes,
            "prepared_resident_count":prepared, "calibration_refusal":refusal,
            "whole_runtime_factory_called":false, "runtime_admission":false, "hardware_effects":false,
        })
    );
}
