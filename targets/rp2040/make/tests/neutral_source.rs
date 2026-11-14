//! Actual neutral board modules and ordinary Make emissions, CPU refusal proof.
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
#[ignore = "requires ordinary Make neutral OUT_DIR; CPU preparation/refusal proof only"]
fn actual_board_factory_and_root_preflight_refuse_unadmitted_commands() {
    let out = fs::canonicalize(
        std::env::var_os("CONDUIT_NEUTRAL_CPU_OUT_DIR")
            .expect("explicit ordinary Make neutral OUT_DIR"),
    )
    .unwrap();
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
        fs::write(&path, source).unwrap();
        let mut compiler = Command::new("rustc");
        compiler
            .args(["--edition=2021", "--crate-type", kind, "-L"])
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
            assert!(
                ran.status.success(),
                "{}",
                String::from_utf8_lossy(&ran.stderr)
            );
        }
    }
    fs::remove_dir_all(temporary).unwrap();
}
