use std::{fs, path::Path};

#[test]
fn portable_signal_source_refuses_concrete_target_and_topology_owners() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for forbidden in [
        "esp32_wroom.rs",
        "esp32_c3.rs",
        "esp32_s3.rs",
        "std_esp32_bluetooth.rs",
        "std_pico_usb.rs",
        "std_pico_bluetooth.rs",
        "distributed_identity.rs",
        "distributed_plan.rs",
        "triple.rs",
    ] {
        assert!(
            !source.join(forbidden).exists(),
            "concrete Signal owner returned to {}",
            source.join(forbidden).display()
        );
    }

    let production = fs::read_dir(&source)
        .expect("Signal source directory")
        .map(|entry| fs::read_to_string(entry.expect("source entry").path()).expect("Rust source"))
        .collect::<String>();
    for forbidden_fact in [
        "s4/std-source",
        "s4/browser-sink",
        "s4/pico-local",
        "esp32/wroom",
        "esp32/c3",
        "esp32/s3",
        "std-pico-usb",
        "std-esp32-gatt",
    ] {
        assert!(
            !production.contains(forbidden_fact),
            "portable Signal source contains concrete proof fact {forbidden_fact}"
        );
    }
}
