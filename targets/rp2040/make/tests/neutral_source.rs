//! Actual neutral board modules and ordinary Make emissions, CPU preparation proof.
//! Run explicitly with CONDUIT_NEUTRAL_CPU_OUT_DIR pointing at the selected
//! neutral firmware build OUT_DIR. This never opens hardware or starts a Play.
use conduit_mcu_source_build as source_build;
#[path = "../../../../architecture/plot/tests/common/compiled_libraries.rs"]
mod compiled_libraries;
// This CPU adapter uses only one part of the shared protocol proof fixture.
#[allow(dead_code)]
#[path = "../../../mcu/source-build/tests/fixtures/environmental_exchange_source.rs"]
mod environmental_exchange_source;
use std::{fs, path::Path, process::Command};
fn constant<'a>(source: &'a str, name: &str) -> &'a str {
    let prefix = format!("pub const {name}:");
    let matches: Vec<_> = source
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .collect();
    assert_eq!(matches.len(), 1, "one emitted constant {name}");
    matches[0]
        .split_once('=')
        .unwrap()
        .1
        .trim()
        .strip_suffix(';')
        .unwrap()
}
#[test]
#[ignore = "requires ordinary Make neutral OUT_DIR; full CPU preparation only, no Start"]
fn actual_board_factory_and_root_preflight_prepare_complete_checked_source_without_start() {
    let out = fs::canonicalize(
        std::env::var_os("CONDUIT_NEUTRAL_CPU_OUT_DIR")
            .expect("explicit ordinary Make neutral OUT_DIR"),
    )
    .unwrap();
    let build_output_path = out.parent().unwrap().join("output");
    let build_output = fs::read_to_string(&build_output_path).unwrap();
    assert_eq!(
        build_output
            .lines()
            .filter(|line| *line == "cargo:rustc-cfg=neutral_source_execution")
            .count(),
        1,
        "CPU profile must match the original firmware build's emitted cfg"
    );
    let output_digest = conduit_core::assigned_plan_payload_digest(build_output.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    eprintln!("original_build_cfg_witness={:?} original_build_cfg_witness_sha256={output_digest} propagated_cfg=neutral_source_execution", build_output_path);
    let host = fs::read_to_string(out.join("host.rs")).unwrap();
    let fixture = environmental_exchange_source::selected_source();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let board = root.join("../firmware/host/src");
    let config = format!(
        "pub const IMAGE_ID:&str={};pub const CONFIGURATION_ID:&str={};pub const MAX_ASSIGNMENT_BYTES:usize={};pub const I2C_CALL_MAXIMUM_OPERATIONS:u32={};pub const CLOCK_AT_MAXIMUM_CALL_OPERATIONS:u32={};",
        constant(&host,"IMAGE_ID"),constant(&host,"CONFIGURATION_ID"),constant(&host,"MAX_ASSIGNMENT_BYTES"),
        constant(&host,"I2C_CALL_MAXIMUM_OPERATIONS"),constant(&host,"CLOCK_AT_MAXIMUM_CALL_OPERATIONS"));
    let extra = [
        (
            "PREPARATION_DELEGATION",
            "Option<conduit_body::HostPreparationDelegation>",
        ),
        (
            "EXECUTION_DELEGATION",
            "Option<conduit_body::HostExecutionDelegation>",
        ),
        ("RESIDENT_WORK_POOL", "&str"),
        ("RESIDENT_WORK_UNITS", "u32"),
        ("I2C_CONTROLLER", "u8"),
        ("I2C_BASE_IMPLEMENTATION", "Option<&str>"),
        ("I2C_CALL_ADDRESS_RANGE", "Option<(u8,u8)>"),
        ("CLOCK_AT_BASE_IMPLEMENTATION", "Option<&str>"),
        ("HEAP_BYTES", "usize"),
    ]
    .into_iter()
    .map(|(name, kind)| format!("pub const {name}:{kind}={};", constant(&host, name)))
    .collect::<String>();
    let mut generated = format!("extern crate alloc;mod config{{{config}{extra}}}\n");
    for name in [
        "neutral_equipment_owners",
        "neutral_workset",
        "neutral_authority",
        "neutral_factory",
        "neutral_preflight",
        "resident",
    ] {
        generated.push_str(&format!(
            "#[allow(dead_code)]#[path={:?}]mod {name};\n",
            board.join(format!("{name}.rs"))
        ));
    }
    generated
        .push_str("mod neutral_equipment{pub use crate::neutral_equipment_owners::Equipment;}\n");
    generated.push_str(&format!(
        "const EXPECTED_SOURCE_SHA256:[u8;32]={:?};\n",
        fixture.compile().unwrap().sha256
    ));
    let probe = root
        .join("../../../architecture/plot/tests/common/allocation_probe.rs")
        .canonicalize()
        .unwrap();
    let positive = root
        .join("tests/neutral_source/positive.rs")
        .canonicalize()
        .unwrap();
    let exercise = format!("#[path={positive:?}] mod positive;\nmod probe{{include!({probe:?});pub fn current()->usize{{OBSERVATION.with(|o|o.get().live_bytes)}}pub fn snapshot()->Observation{{OBSERVATION.with(|o|o.get())}}}}\n#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;\n{}",include_str!("neutral_source/exercise.rs"));
    let temporary =
        std::env::temp_dir().join(format!("conduit-neutral-source-cpu-{}", std::process::id()));
    fs::create_dir(&temporary).unwrap();
    // CPU-only observation: preserve each original refusal and all checks. The
    // no_std compilation still consumes the unmodified production factory.
    let original_factory_path = board.join("neutral_factory.rs");
    let original_factory = fs::read_to_string(&original_factory_path).unwrap();
    let digest = |text: &str| {
        conduit_core::assigned_plan_payload_digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let diagnose = match std::env::var("CONDUIT_NEUTRAL_DIAGNOSE_KERNEL") {
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) if value == "1" => true,
        _ => panic!("CONDUIT_NEUTRAL_DIAGNOSE_KERNEL accepts only explicit1"),
    };
    let observed_factory_path = if diagnose {
        let mut observed_factory = original_factory.clone();
        for stage in ["neutral-kernel", "neutral-workspace"] {
            let erased = format!(".map_err(|_| \"{stage}\")?");
            assert_eq!(observed_factory.matches(&erased).count(), 1);
            let observed = format!(".map_err(|error| {{ eprintln!(\"original factory {stage}: {{error:?}}\"); \"{stage}\" }})?");
            observed_factory = observed_factory.replace(&erased, &observed);
        }
        let stock_marker =
            "            // Kernel events and the Plan's named mandatory-Sign receipts are";
        assert_eq!(observed_factory.matches(stock_marker).count(), 1);
        let stock_report = r#"
            let diagnostic_canonical_payload: u64 = layout::CORD_VALUE_PROFILES.iter()
                .chain(layout::STEP_VALUE_PROFILES)
                .filter_map(|p| match p {
                    conduit_mcu_host_runtime::native_values::ValueSlotProfile::Canonical(n) => Some(u64::from(*n)),
                    _ => None,
                }).sum::<u64>() + layout::HOST_CALL_VALUE_SLOT_CAPACITIES.iter().map(|n| u64::from(*n)).sum::<u64>();
            eprintln!("selected_stock canonical_slots={} schema_logical_slots={} schema_physical_slots={} reduced_classes={} unknown_fallback={} canonical_payload_bytes={} schema_payload_bytes={} schema_metadata_bytes={} canonical_backing_cpu_bytes={}",
                conduit_kernel::ValueStorage::item_capacity(values.canonical_storage()),
                _stock.logical_schema_slots, _stock.physical_schema_slots, _stock.reduced_classes,
                _stock.unknown_owner_fallback, diagnostic_canonical_payload, values.reserved_payload_bytes(),
                values.reserved_metadata_bytes(), values.canonical_storage().reserved_storage_bytes());
"#;
        observed_factory =
            observed_factory.replace(stock_marker, &format!("{stock_report}{stock_marker}"));
        let preparation = "            let mut prepared = PreparedFragment";
        assert_eq!(observed_factory.matches(preparation).count(), 1);
        let reports = r#"
                let mut diagnostic_reports = [[None; P]; N];
                let available = conduit_mcu_host_runtime::execution::PreparedBacks::<N,P>::write_terminal_transductions(&backs, &mut diagnostic_reports);
                eprintln!("original factory terminal report available={} source_nodes={N} ports={P}", available.is_some());
                for node in 0..N {
                    for port in 0..P {
                        if diagnostic_reports[node][port] != TERMINALS[node][port] {
                            eprintln!("original factory terminal mismatch node={node} port={port} actual={:?} expected={:?}", diagnostic_reports[node][port], TERMINALS[node][port]);
                        }
                    }
                }
    "#;
        observed_factory =
            observed_factory.replace(preparation, &format!("{reports}{preparation}"));
        let observed_factory_path = temporary.join("neutral_factory_diagnostic.rs");
        fs::write(&observed_factory_path, &observed_factory).unwrap();
        eprintln!("factory_original_sha256={} factory_diagnostic_sha256={} diagnostic_only=error_and_immutable_terminal_and_stock_observation original_checks_and_refusals_preserved",
            digest(&original_factory), digest(&observed_factory));
        Some(observed_factory_path)
    } else {
        eprintln!(
            "factory_original_sha256={} factory_diagnostic_copy=false",
            digest(&original_factory)
        );
        None
    };
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let libraries = compiled_libraries::resolve(
        &deps,
        &[
            "conduit_core",
            "conduit_plot",
            "conduit_seven_segment_mask",
            "conduit_body",
            "conduit_mcu_host_runtime",
            "conduit_kernel",
            "conduit_plan_lowering",
            "conduit_semantic_catalog",
            "conduit_i2c_transaction",
            "conduit_monotonic_wait",
            "conduit_environment_runtime",
            "heapless",
            "serde_json",
            "critical_section",
            "static_cell",
            "ed25519_dalek",
        ],
    );
    for (name, source, kind) in [
        ("image", format!("{generated}{exercise}"), "bin"),
        ("no_std", format!("#![no_std]\n{generated}"), "rlib"),
    ] {
        let path = temporary.join(format!("{name}.rs"));
        let source = match (kind, observed_factory_path.as_ref()) {
            ("bin", Some(observed_factory_path)) => {
                let original = format!("#[path={original_factory_path:?}]mod neutral_factory;");
                assert_eq!(source.matches(&original).count(), 1);
                source.replace(
                    &original,
                    &format!("#[path={observed_factory_path:?}]mod neutral_factory;"),
                )
            }
            _ => source,
        };
        fs::write(&path, source).unwrap();
        let mut compiler = Command::new("rustc");
        compiler
            .args([
                "--edition=2021",
                "--crate-type",
                kind,
                "--cfg",
                "neutral_source_execution",
                "--check-cfg",
                "cfg(neutral_source_execution)",
                "-L",
            ])
            .arg(format!("dependency={}", deps.display()))
            .env("OUT_DIR", &out);
        for library in &libraries {
            compiler.arg("--extern").arg(library);
        }
        let executable = temporary.join(name);
        let built = compiler
            .arg(path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        if kind == "bin" {
            let ran = Command::new(executable).output().unwrap();
            eprintln!("{}", String::from_utf8_lossy(&ran.stdout));
            if diagnose {
                eprintln!("{}", String::from_utf8_lossy(&ran.stderr));
            }
            assert!(
                ran.status.success(),
                "{}",
                String::from_utf8_lossy(&ran.stderr)
            );
        }
    }
    fs::remove_dir_all(temporary).unwrap();
}
