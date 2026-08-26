use std::{env, fs, path::PathBuf};

use conduit_embedded_build::{EmbeddedImageBounds, generate_embedded_plan};
use conduit_host_esp32_fabrication::{
    esp32_descriptor_binding, hw463_esp_wroom_32_sample, validate_esp32_descriptor,
};
use conduit_runtime::lowering::{MAXIMUM_KERNEL_PORTS_PER_NODE, lower_plan_fragment};
use conduit_signal::{
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, ESP32_WROOM_PHYSICAL_HOST_ID, SIGNAL_ENCODED_LEN,
    exact_std_esp32_bluetooth_plan,
};

fn main() {
    println!("cargo:rerun-if-changed=../../fixtures/forms/signal-demo.conduit");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    let descriptor = hw463_esp_wroom_32_sample();
    validate_esp32_descriptor(&descriptor)
        .expect("inspected WROOM fabrication descriptor must remain valid");
    let descriptor_binding = esp32_descriptor_binding(&descriptor)
        .expect("inspected WROOM fabrication descriptor must have an exact binding");
    let plan = exact_std_esp32_bluetooth_plan([0; 6])
        .expect("the inspected WROOM Bluetooth image must plan")
        .plan;
    let fragment = plan
        .fragments
        .iter()
        .find(|candidate| candidate.host_id.as_str() == ESP32_WROOM_PHYSICAL_HOST_ID)
        .expect("WROOM plan must contain its exact fragment");
    let lowered = lower_plan_fragment(fragment).expect("WROOM fragment must lower");
    let generated = generate_embedded_plan(fragment, &lowered, image_bounds())
        .expect("WROOM fragment must fit reviewed image bounds");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let mut module = generated.render_no_alloc_firmware_module();
    module.push_str(&format!(
        "\npub const GENERATED_FABRICATION_DESCRIPTOR_BINDING: &str = {descriptor_binding:?};\n"
    ));
    fs::write(out.join("signal_image.rs"), module).expect("generated WROOM image must be writable");
}

fn image_bounds() -> EmbeddedImageBounds {
    EmbeddedImageBounds {
        maximum_nodes: 1,
        maximum_cords: 1,
        maximum_routes: 1,
        maximum_route_targets: 1,
        maximum_host_operations: 2,
        maximum_resources: 2,
        maximum_sign_expectations: 8,
        maximum_configuration_entries: 0,
        maximum_ports_per_node: MAXIMUM_KERNEL_PORTS_PER_NODE,
        maximum_remote_endpoints: 1,
        maximum_cord_value_slots: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        maximum_cord_value_bytes: SIGNAL_ENCODED_LEN,
        maximum_sign_items: 16,
        maximum_sign_bytes: 1024,
    }
}
