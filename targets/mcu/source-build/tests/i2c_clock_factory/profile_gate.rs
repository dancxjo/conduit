//! Fresh checked Host/Source fabrication gate, not runtime authority or MCU fit.
//! Compile against one coherent ordinary Make graph; no toolchain target build.
use conduit_mcu_source_build as source_build;
#[allow(dead_code)]
#[path = "../fixtures/environmental_exchange_source.rs"]
mod environmental_source;
use conduit_host_make as h;

fn checked(source: &str) -> h::CheckedHostConfiguration {
    let packages = h::MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let catalog = h::MakeCatalog::canonical().with_packages(&packages);
    h::check_host_configuration(
        h::parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap()
}
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("explicit repository"));
    let emitted = std::path::PathBuf::from(std::env::args_os().nth(2).expect("explicit output"));
    std::fs::create_dir_all(&emitted).unwrap();
    source_build::source_composition::emit_to(&emitted).unwrap();
    assert_eq!(
        std::fs::read(emitted.join("source_native_i2c_clock_factory.rs")).unwrap(),
        std::fs::read(
            root.join(
                "targets/mcu/source-build/src/source_composition/native_i2c_clock_factory.rs"
            )
        )
        .unwrap()
    );
    let baseline = checked(
        &std::fs::read_to_string(
            root.join("targets/rp2040/profiles/pico-w-house-green-delegated.host.conduit"),
        )
        .unwrap(),
    );
    let candidate = checked(
        &std::fs::read_to_string(
            root.join("targets/rp2040/profiles/pico-w-house-green-source-candidate.host.conduit"),
        )
        .unwrap(),
    );
    assert_ne!(baseline.configuration_id(), candidate.configuration_id());
    assert_eq!(
        baseline.configuration().limits,
        candidate.configuration().limits
    );
    assert_eq!(
        baseline.configuration().resources,
        candidate.configuration().resources
    );
    let missing = source_build::clock::CheckedClockConstruction::from_configuration(
        &baseline,
        conduit_host_rp2040::CLOCK_IMPLEMENTATION,
    )
    .unwrap_err();
    assert!(missing.contains("missing monotonic clock construction"));
    let clock = source_build::clock::CheckedClockConstruction::from_configuration(
        &candidate,
        conduit_host_rp2040::CLOCK_IMPLEMENTATION,
    )
    .unwrap();
    assert_eq!(clock.deadline_call_operations().unwrap(), 1024);
    let work =
        conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&candidate).unwrap();
    let source = environmental_source::selected_source().compile().unwrap();
    let result = source.prepare_selected_residents_with_exact_work(
        source_build::resident_expression_bounds(u16::try_from(work.capacity_units).unwrap()),
        conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema,
        work.capacity_units,
    );
    // Compiler-only requirement inspection, not a pool admission: retain the
    // SAME per-expression ceiling and inspect actual computation_work reports.
    // These conservative artifacts are not supplied to any runtime or planner.
    let diagnostic = source.prepare_selected_residents_with_storage(
        source_build::resident_expression_bounds(u16::try_from(work.capacity_units).unwrap()),
        conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema,
    );
    let (required, resident_count, diagnostic_refusal) = match diagnostic {
        Ok(residents) => (
            Some(
                residents
                    .iter()
                    .map(|r| u32::from(r.computation_work()))
                    .sum::<u32>(),
            ),
            Some(residents.len()),
            None,
        ),
        Err(error) => (None, None, Some(error)),
    };
    // Explicit compiler-only measurement ceiling. It is the existing compiler
    // representation maximum, NOT a Host pool or runtime admission. Artifacts
    // produced here are inspected and dropped, never used for calibration.
    let full = source.prepare_selected_residents_with_storage(
        source_build::resident_expression_bounds(u16::MAX - 3),
        conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema,
    );
    let (full_required, full_count, largest, full_refusal) = match full {
        Ok(residents) => (
            Some(
                residents
                    .iter()
                    .map(|r| u32::from(r.computation_work()))
                    .sum::<u32>(),
            ),
            Some(residents.len()),
            residents.iter().map(|r| r.computation_work()).max(),
            None,
        ),
        Err(error) => (None, None, None, Some(error)),
    };
    let owned = source.prepare_selected_residents_with_storage(
        source_build::resident_expression_bounds(u16::MAX - 3),
        conduit_embedded_build::ResidentExecutionStorage::Owned,
    );
    let (owned_required, owned_count, owned_largest, owned_refusal) = match owned {
        Ok(residents) => (
            Some(
                residents
                    .iter()
                    .map(|r| u32::from(r.computation_work()))
                    .sum::<u32>(),
            ),
            Some(residents.len()),
            residents.iter().map(|r| r.computation_work()).max(),
            None,
        ),
        Err(error) => (None, None, None, Some(error)),
    };
    let (prepared, refusal) = match result {
        Ok(residents) => (residents.len(), None),
        Err(error) => (0, Some(error)),
    };
    println!(
        "{}",
        serde_json::json!({
            "scope":"fresh checked open-Fore BME/HT protocol service; no autonomous sampling or Face/Mask",
            "baseline_configuration":baseline.configuration_id(),
            "candidate_configuration":candidate.configuration_id(),
            "baseline_missing_clock":missing,
            "memory_work_operation_bounds_preserved":true,
        "emitted_original_factory_bytes":true,
            "clock_slots":clock.slots,
            "clock_maximum_operations":clock.maximum_call_operations,
            "resident_pool_units":work.capacity_units,
        "compiler_only_total_work_requirement":required,
        "compiler_only_resident_count":resident_count,
        "compiler_only_measurement_refusal":diagnostic_refusal,
        "full_compiler_only_ceiling":u16::MAX-3,
        "full_compiler_only_work_requirement":full_required,
        "full_compiler_only_resident_count":full_count,
        "full_compiler_only_largest_owner":largest,
        "full_compiler_only_refusal":full_refusal,
        "owned_compiler_only_work_requirement":owned_required,
        "owned_compiler_only_resident_count":owned_count,
        "owned_compiler_only_largest_owner":owned_largest,
        "owned_compiler_only_refusal":owned_refusal,
            "prepared_resident_count":prepared,
            "resident_preparation_refusal":refusal,
            "whole_factory_called":false,
            "runtime_admission":false,
            "hardware_effects":false,
        "autonomous_sampling":false, "standard_Face_Mask":false,
        })
    );
}
