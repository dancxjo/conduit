use std::{env, fmt::Write, fs, path::PathBuf};

use conduit_embedded_build::{EmbeddedImageBounds, generate_embedded_plan};
use conduit_host_esp32_fabrication::{
    esp32_descriptor_binding, validate_esp32_descriptor, Esp32BoardDescriptor,
};
use conduit_runtime::lowering::{MAXIMUM_KERNEL_PORTS_PER_NODE, lower_plan_fragment};
use conduit_signal::{
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, ESP32_C3_PHYSICAL_HOST_ID, SIGNAL_ENCODED_LEN,
    exact_std_esp32_c3_bluetooth_plan,
};

fn main() {
    println!("cargo:rerun-if-changed=../../fixtures/forms/signal-demo.conduit");
    println!("cargo:rerun-if-changed=board-descriptor.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    let descriptor: Esp32BoardDescriptor = serde_json::from_str(
        &fs::read_to_string("board-descriptor.json").expect("C3 descriptor must be readable"),
    )
    .expect("C3 descriptor must decode");
    validate_esp32_descriptor(&descriptor)
        .expect("the inspected C3 fabrication descriptor must remain valid");
    let descriptor_binding = esp32_descriptor_binding(&descriptor)
        .expect("the inspected C3 fabrication descriptor must have an exact binding");
    let distributed = env::var_os("CARGO_FEATURE_DISTRIBUTED_LENIA").is_some();
    let plan = if distributed {
        conduit_alife::exact_distributed_lenia_plan()
            .expect("the distributed Lenia image must plan").plan
    } else {
        exact_std_esp32_c3_bluetooth_plan([0; 6])
            .expect("the inspected C3 Bluetooth image must plan").plan
    };
    let fragment = plan
        .fragments
        .iter()
        .find(|candidate| candidate.host_id.as_str() == ESP32_C3_PHYSICAL_HOST_ID)
        .expect("C3 plan must contain its exact fragment");
    let lowered = lower_plan_fragment(fragment).expect("C3 fragment must lower");
    let generated = generate_embedded_plan(fragment, &lowered, image_bounds(distributed))
        .expect("C3 fragment must fit reviewed image bounds");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let mut module = generated.render_no_alloc_firmware_module();
    module.push_str(&format!(
        "\npub const GENERATED_FABRICATION_DESCRIPTOR_BINDING: &str = {descriptor_binding:?};\n"
    ));
    if distributed {
        let bindings = conduit_alife::distributed_lenia_participant_bindings(
            &plan, conduit_alife::DISTRIBUTED_LENIA_C3_HOST_ID,
            conduit_alife::DISTRIBUTED_LENIA_C3_BOOT_ID,
        ).expect("C3 Lenia bindings must resolve");
        render_lenia_bindings(&mut module, &bindings);
    }
    fs::write(out.join("signal_image.rs"), module).expect("generated C3 image must be writable");
}

fn image_bounds(distributed: bool) -> EmbeddedImageBounds {
    EmbeddedImageBounds {
        maximum_nodes: 1,
        maximum_cords: if distributed { 2 } else { 1 },
        maximum_routes: if distributed { 2 } else { 1 },
        maximum_route_targets: if distributed { 2 } else { 1 },
        maximum_host_operations: if distributed { 0 } else { 2 },
        maximum_resources: if distributed { 0 } else { 2 },
        maximum_sign_expectations: 8,
        maximum_configuration_entries: 0,
        maximum_ports_per_node: MAXIMUM_KERNEL_PORTS_PER_NODE,
        maximum_remote_endpoints: if distributed { 2 } else { 1 },
        maximum_cord_value_slots: if distributed { 2 } else { DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS },
        maximum_cord_value_bytes: if distributed { conduit_alife::DISTRIBUTED_LENIA_VALUE_BYTES * 2 } else { SIGNAL_ENCODED_LEN },
        maximum_sign_items: 16,
        maximum_sign_bytes: 1024,
    }
}

fn render_lenia_bindings(module: &mut String, bindings: &conduit_alife::DistributedLeniaParticipantBindings) {
    for (prefix, binding) in [("WORK", &bindings.work), ("RESULT", &bindings.result)] {
        for (name, value) in [
            ("PLAY_ID", binding.play_id.as_str()), ("LINE_ID", binding.line_id.as_str()),
            ("SOURCE_HOST_ID", binding.source_host_id.as_str()), ("SOURCE_BOOT_ID", binding.source_boot_id.as_str()),
            ("SINK_HOST_ID", binding.sink_host_id.as_str()), ("SINK_BOOT_ID", binding.sink_boot_id.as_str()),
        ] { writeln!(module, "pub const LENIA_{prefix}_{name}: &str = {value:?};").unwrap(); }
    }
}
