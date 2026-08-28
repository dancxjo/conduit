use std::{env, fmt::Write, fs, path::PathBuf};

use conduit_embedded_build::{EmbeddedImageBounds, generate_embedded_plan};
use conduit_host_esp32_fabrication::{
    esp32_descriptor_binding, hw463_esp_wroom_32_sample, validate_esp32_descriptor,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};
use conduit_signal::SIGNAL_ENCODED_LEN;
use conduit_signal_conformance::{
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, ESP32_WROOM_PHYSICAL_HOST_ID,
    exact_std_esp32_bluetooth_plan,
};

fn main() {
    println!("cargo:rerun-if-changed=../../../../proof/fixtures/forms/signal-demo.conduit");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    let descriptor = hw463_esp_wroom_32_sample();
    validate_esp32_descriptor(&descriptor)
        .expect("inspected WROOM fabrication descriptor must remain valid");
    let descriptor_binding = esp32_descriptor_binding(&descriptor)
        .expect("inspected WROOM fabrication descriptor must have an exact binding");
    let distributed = env::var_os("CARGO_FEATURE_DISTRIBUTED_LENIA").is_some();
    let plan = if distributed {
        conduit_alife_distributed_conformance::exact_distributed_lenia_plan()
            .expect("the distributed Lenia image must plan")
            .plan
    } else {
        exact_std_esp32_bluetooth_plan([0; 6])
            .expect("the inspected WROOM Bluetooth image must plan")
            .plan
    };
    let fragment = plan
        .fragments
        .iter()
        .find(|candidate| candidate.host_id.as_str() == ESP32_WROOM_PHYSICAL_HOST_ID)
        .expect("WROOM plan must contain its exact fragment");
    let lowered = lower_plan_fragment(fragment).expect("WROOM fragment must lower");
    let generated = generate_embedded_plan(fragment, &lowered, image_bounds(distributed))
        .expect("WROOM fragment must fit reviewed image bounds");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let mut module = generated.render_no_alloc_firmware_module();
    module.push_str(&format!(
        "\npub const GENERATED_FABRICATION_DESCRIPTOR_BINDING: &str = {descriptor_binding:?};\n"
    ));
    if distributed {
        let bindings =
            conduit_alife_distributed_conformance::distributed_lenia_participant_bindings(
                &plan,
                conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_WROOM_HOST_ID,
                conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_WROOM_BOOT_ID,
            )
            .expect("WROOM Lenia bindings must resolve");
        render_lenia_bindings(&mut module, &bindings);
    }
    fs::write(out.join("signal_image.rs"), module).expect("generated WROOM image must be writable");
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
        maximum_ports_per_node: FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
        maximum_remote_endpoints: if distributed { 2 } else { 1 },
        maximum_cord_value_slots: if distributed {
            2
        } else {
            DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS
        },
        maximum_cord_value_bytes: if distributed {
            conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_VALUE_BYTES * 2
        } else {
            SIGNAL_ENCODED_LEN
        },
        maximum_sign_items: 16,
        maximum_sign_bytes: 1024,
    }
}

fn render_lenia_bindings(
    module: &mut String,
    bindings: &conduit_alife_distributed_conformance::DistributedLeniaParticipantBindings,
) {
    for (prefix, binding) in [("WORK", &bindings.work), ("RESULT", &bindings.result)] {
        for (name, value) in [
            ("PLAY_ID", binding.play_id.as_str()),
            ("LINE_ID", binding.line_id.as_str()),
            ("SOURCE_HOST_ID", binding.source_host_id.as_str()),
            ("SOURCE_BOOT_ID", binding.source_boot_id.as_str()),
            ("SINK_HOST_ID", binding.sink_host_id.as_str()),
            ("SINK_BOOT_ID", binding.sink_boot_id.as_str()),
        ] {
            writeln!(module, "pub const LENIA_{prefix}_{name}: &str = {value:?};").unwrap();
        }
    }
}
