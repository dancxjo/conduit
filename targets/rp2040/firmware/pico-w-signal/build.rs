use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, BaseImplementationId, BootId, HostId,
    PlacementId, PlanId,
};
use conduit_embedded_build::{generate_embedded_plan, EmbeddedImageBounds, GeneratedEmbeddedPlan};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_signal::{signal_profile_catalog, SHOW_KIND, SIGNAL_ENCODED_LEN};
use conduit_signal_conformance::{
    exact_std_pico_bluetooth_plan, exact_std_pico_usb_plan, pico_local_advertisement, triple,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, PICO_LOCAL_HOST_ID, STD_PICO_USB_SINK_HOST_ID,
};

mod identity_sidecar;
use identity_sidecar::{render_network_identity_sidecar, render_signal_identity_sidecar};
#[path = "build/firmware_mode.rs"]
mod firmware_mode;
use firmware_mode::firmware_mode;
#[path = "build/r1_control_images.rs"]
mod r1_control_images;

const SIGNAL_DEMO_FORM: &str = include_str!("../../../../proof/fixtures/forms/signal-demo.conduit");
const R1_CONTROL_FORM: &str = include_str!("../../../../proof/fixtures/forms/r1-three-peer-control.conduit");
const TRIPLE_SIGNAL_FORM: &str = include_str!("../../../../proof/fixtures/forms/triple-signal.conduit");
const IDENTITY_SIDECAR_ENV: &str = "CONDUIT_PICO_SIGNAL_IDENTITY_SIDECAR";
const IDENTITY_SIDECAR_RERUN_ENV: &str = "CONDUIT_PICO_SIGNAL_IDENTITY_RERUN";
const APPLIANCE_IDENTITY_SIDECAR_ENV: &str = "CONDUIT_PICO_APPLIANCE_IDENTITY_SIDECAR";
const APPLIANCE_HIL_CLIENT_IDENTITY_SIDECAR_ENV: &str =
    "CONDUIT_PICO_APPLIANCE_HIL_CLIENT_IDENTITY_SIDECAR";
const APPLIANCE_HIL_CLIENT_ARTIFACT: &str = "pico/appliance-hil-client-firmware@1";
const DISTRIBUTED_LENIA_ARTIFACT: &str = "pico/distributed-lenia-worker@1";
const MAX_STORED_SIGNAL_VALUES: usize = 16;
const WAIT_VALUE_BYTES: u32 = 8;
const RUNTIME_SIGN_EVENTS: usize = 256;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let appliance_build_id = appliance_build_id();
    println!("cargo:rustc-env=CONDUIT_PICO_APPLIANCE_BUILD_ID={appliance_build_id}");
    println!("cargo:rerun-if-env-changed=CONDUIT_PICO_INDICATOR_IDENTITY_SIDECAR");
    if firmware_mode() == "indicator-resource" {
        if let Some(path) = env::var_os("CONDUIT_PICO_INDICATOR_IDENTITY_SIDECAR") {
            fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "conduit.pico-indicator/image@1",
                "firmware_mode": "indicator-resource",
                "firmware_build_id": appliance_build_id,
                "git_revision": git_revision(),
                "tree_state": git_tree_state(),
                "target": env::var("TARGET").unwrap(),
                "profile": env::var("PROFILE").unwrap(),
                "conduit_plan_claimed": false,
            })).unwrap()).expect("indicator build identity must be writable");
        }
    }
    println!(
        "cargo:rustc-env=CONDUIT_PETE_CAPSTONE_BUILD_ID={}",
        pete_capstone_build_id()
    );
    if firmware_mode() == "pete-capstone" {
        emit_linker_contract(&out);
        return;
    }
    generate_body_advertisement(&out, firmware_mode());

    if firmware_mode() == "appliance-hello" {
        generate_pico_appliance_identity();
    } else if firmware_mode() == "appliance-hil-client" {
        generate_pico_appliance_hil_client_identity();
    } else if firmware_mode() == "wifi-bootstrap" {
        generate_pico_network_image(&out);
        generate_r1_recovery_signal_images(&out);
        r1_control_images::generate(&out, false);
    } else if firmware_mode() == "r1-control" {
        generate_pico_network_image(&out);
        generate_r1_recovery_signal_images(&out);
        r1_control_images::generate(&out, true);
    } else if firmware_mode() == "distributed-lenia" {
        generate_pico_lenia_image(&out);
    } else {
        generate_pico_signal_image(&out);
    }

    emit_linker_contract(&out);
}

fn generate_pico_lenia_image(out: &Path) {
    let exact = conduit_alife_distributed_conformance::exact_distributed_lenia_plan()
        .expect("the exact distributed Lenia Plan must resolve");
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment.host_id.as_str()
                == conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_PICO_HOST_ID
        })
        .expect("distributed Lenia Plan must contain the Pico worker");
    let lowered = lower_plan_fragment(fragment).expect("Pico Lenia fragment must lower");
    let generated = generate_embedded_plan(
        fragment,
        &lowered,
        EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 2,
            maximum_routes: 2,
            maximum_route_targets: 2,
            maximum_host_operations: 0,
            maximum_resources: 0,
            maximum_sign_expectations: 8,
            maximum_configuration_entries: 0,
            maximum_ports_per_node:
                conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            maximum_remote_endpoints: 2,
            maximum_cord_value_slots: 2,
            maximum_cord_value_bytes:
                conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_VALUE_BYTES * 2,
            maximum_sign_items: 16,
            maximum_sign_bytes: 1024,
        },
    )
    .expect("Pico Lenia fragment must fit reviewed fixed-image bounds");
    let bindings = conduit_alife_distributed_conformance::distributed_lenia_participant_bindings(
        &exact.plan,
        conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_PICO_HOST_ID,
        conduit_alife_distributed_conformance::DISTRIBUTED_LENIA_PICO_BOOT_ID,
    )
    .expect("Pico Lenia bindings must resolve");
    let mut module = generated.render_no_alloc_firmware_module();
    render_lenia_binding_constants(&mut module, &bindings);
    writeln!(
        module,
        "pub const LENIA_FIRMWARE_BUILD_ID: &str = {:?};",
        appliance_build_id()
    )
    .expect("writing to a String cannot fail");
    fs::write(out.join("pico_signal_image.rs"), module)
        .expect("generated Pico Lenia image must be writable");
    let active_play =
        bind_active_play(&exact.plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let sidecar = serde_json::json!({
        "schema": "conduit.distributed-lenia/generated-worker-image@1",
        "firmware_mode": "distributed-lenia",
        "firmware_build_id": appliance_build_id(),
        "source_document_id": exact.plan.source_document_id.as_str(),
        "checked_form_id": exact.plan.checked_form_id.as_str(),
        "expanded_form_id": exact.plan.expanded_form_id.as_str(),
        "plan_id": generated.plan_id,
        "fragment_id": generated.fragment_id,
        "host_id": generated.host_id,
        "boot_id": generated.boot_id,
        "active_play_id": active_play.active_play_id.as_str(),
        "boot_sign_id": bind_sign(&fragment.host_id, &fragment.boot_id, None, 0).sign_id.as_str(),
        "presentation_ids": [],
        "presentation_sign_ids": [],
        "terminal_sign_id": bind_sign(&fragment.host_id, &fragment.boot_id, Some(&active_play.active_play_id), 0).sign_id.as_str(),
        "offer_generation": generated.offer_generation,
        "nodes": generated.nodes.len(),
        "cords": generated.cords.len(),
        "host_operations": generated.host_operations.len(),
        "cord_value_slots": generated.cord_value_slots,
        "cord_value_bytes": generated.cord_value_bytes,
        "sign_items": generated.sign_items,
        "sign_bytes": generated.sign_bytes,
    });
    let sidecar =
        serde_json::to_string_pretty(&sidecar).expect("distributed Lenia identity must serialize");
    fs::write(out.join("pico_signal_identity.json"), &sidecar)
        .expect("generated Pico Lenia identity sidecar must be writable");
    if let Ok(path) = env::var(IDENTITY_SIDECAR_ENV) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Lenia sidecar directory must be writable");
        }
        fs::write(path, sidecar).expect("explicit Lenia identity sidecar must be writable");
    }
}

fn render_lenia_binding_constants(
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
            writeln!(module, "pub const LENIA_{prefix}_{name}: &str = {value:?};")
                .expect("writing to a String cannot fail");
        }
    }
}

fn emit_linker_contract(out: &Path) {
    // Only emit the RP2040-specific linker flags for thumbv6m
    if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "arm"
        && env::var("TARGET").unwrap_or_default() == "thumbv6m-none-eabi"
    {
        let memory_x = include_str!("memory.x");
        fs::write(out.join("memory.x"), memory_x).unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../../../proof/fixtures/forms/signal-demo.conduit");
    println!(
        "cargo:rerun-if-changed=../../../../proof/fixtures/forms/r1-three-peer-control.conduit"
    );
    println!("cargo:rerun-if-changed=../../../../proof/fixtures/forms/triple-signal.conduit");
    println!("cargo:rerun-if-changed=../../../../forms/r1-network-bootstrap/main.conduit");
    println!("cargo:rerun-if-env-changed={IDENTITY_SIDECAR_ENV}");
    r1_control_images::emit_rerun_directives();
    println!("cargo:rerun-if-env-changed={IDENTITY_SIDECAR_RERUN_ENV}");
    println!("cargo:rerun-if-env-changed={APPLIANCE_IDENTITY_SIDECAR_ENV}");
    println!("cargo:rerun-if-env-changed={APPLIANCE_HIL_CLIENT_IDENTITY_SIDECAR_ENV}");
}

fn generate_body_advertisement(out: &Path, mode: &str) {
    let boot_id = BootId::from(
        "conduit-pico-w-signal/runtime-boot:0000000000000000:00000000000000000000000000000000",
    );
    let advertisement = if mode == "r1-control" {
        conduit_r1_network_conformance::r1_signal_pico_advertisement(boot_id)
    } else {
        let mut advertisement = pico_local_advertisement();
        advertisement.boot_id = boot_id;
        advertisement
    };
    println!(
        "cargo:rustc-env=CONDUIT_PICO_BODY_HOST_ID={}",
        advertisement.host_id.as_str()
    );
    fs::write(
        out.join("pico_body_advertisement.json"),
        serde_json::to_vec(&advertisement).expect("Pico Body advertisement must serialize"),
    )
    .expect("generated Pico Body advertisement should be writable");
}

fn appliance_build_id() -> String {
    let artifact = if firmware_mode() == "indicator-resource" {
        "pico/indicator-resource-firmware@1"
    } else if firmware_mode() == "appliance-hil-client" {
        APPLIANCE_HIL_CLIENT_ARTIFACT
    } else if firmware_mode() == "distributed-lenia" {
        DISTRIBUTED_LENIA_ARTIFACT
    } else {
        conduit_rp2040_network_realization::PICO_APPLIANCE_ARTIFACT
    };
    format!(
        "conduit-pico-w-signal:{}:{}:{}:{}:{}:{}",
        git_revision(),
        git_tree_state(),
        env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_owned()),
        env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_owned()),
        firmware_mode(),
        artifact,
    )
}

fn pete_capstone_build_id() -> String {
    format!(
        "conduit-pico-w-pete-capstone:{}:{}:{}:{}:physical-play@1",
        git_revision(),
        git_tree_state(),
        env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_owned()),
        env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_owned()),
    )
}

fn generate_pico_appliance_hil_client_identity() {
    let identity = serde_json::json!({
        "schema": "conduit.pico-appliance/hil-client-image@1",
        "firmware_mode": firmware_mode(),
        "firmware_build_id": appliance_build_id(),
        "image_artifact": APPLIANCE_HIL_CLIENT_ARTIFACT,
        "fixture_only": true,
        "usb_serial": "conduit-pico-hil-client",
        "ssid": conduit_rp2040_network_realization::APPLIANCE_SSID,
        "open_ap": true,
        "server_address": conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS,
        "local_name": conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME,
        "hello_body": conduit_rp2040_network_realization::APPLIANCE_HELLO_BODY,
        "maximum_http_request_bytes": conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES,
        "maximum_http_response_bytes": conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES,
    });
    let sidecar = env::var_os(APPLIANCE_HIL_CLIENT_IDENTITY_SIDECAR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must exist"))
                .join("pico-appliance-hil-client.generated-image.json")
        });
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent)
            .expect("appliance HIL client sidecar directory must be writable");
    }
    fs::write(
        sidecar,
        serde_json::to_string_pretty(&identity)
            .expect("appliance HIL client identity must serialize"),
    )
    .expect("appliance HIL client identity sidecar must be writable");
}

fn generate_pico_appliance_identity() {
    let advertisement = conduit_rp2040_network_realization::pico_appliance_advertisement(
        "pico/appliance-hello",
        "image/boot-bound-at-runtime",
        conduit_rp2040_network_realization::PicoApplianceComposition::Hello,
        conduit_rp2040_network_realization::PicoApplianceInitialization::hello_ready(),
    )
    .expect("complete appliance composition must advertise");
    let identity = serde_json::json!({
        "schema": "conduit.pico-appliance/generated-image@1",
        "firmware_mode": firmware_mode(),
        "firmware_build_id": appliance_build_id(),
        "image_artifact": conduit_rp2040_network_realization::PICO_APPLIANCE_ARTIFACT,
        "service_artifacts": [
            conduit_rp2040_network_realization::AP_SERVICE_ARTIFACT,
            conduit_rp2040_network_realization::DHCP_SERVICE_ARTIFACT,
            conduit_rp2040_network_realization::DNS_SERVICE_ARTIFACT,
            conduit_rp2040_network_realization::HTTP_SERVICE_ARTIFACT,
        ],
        "host_advertisement": advertisement,
        "ssid": conduit_rp2040_network_realization::APPLIANCE_SSID,
        "open_ap": true,
        "channel": 6,
        "server_address": conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS,
        "local_name": conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME,
        "hello_body": conduit_rp2040_network_realization::APPLIANCE_HELLO_BODY,
        "maximum_associations": conduit_rp2040_network_realization::MAXIMUM_AP_ASSOCIATIONS,
        "maximum_dhcp_leases": conduit_rp2040_network_realization::MAXIMUM_DHCP_LEASES,
        "maximum_dhcp_packet_bytes": conduit_rp2040_network_realization::MAXIMUM_DHCP_PACKET_BYTES,
        "maximum_dns_packet_bytes": conduit_rp2040_network_realization::MAXIMUM_DNS_PACKET_BYTES,
        "maximum_http_request_bytes": conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES,
        "maximum_http_response_bytes": conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES,
        "maximum_signs": conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_SIGNS,
        "maximum_network_sockets": conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_NETWORK_SOCKETS,
    });
    let sidecar = env::var_os(APPLIANCE_IDENTITY_SIDECAR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must exist"))
                .join("pico-appliance.generated-image.json")
        });
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent).expect("appliance sidecar directory must be writable");
    }
    fs::write(
        sidecar,
        serde_json::to_string_pretty(&identity).expect("appliance identity must serialize"),
    )
    .expect("appliance identity sidecar must be writable");
}

fn generate_r1_recovery_signal_images(out: &Path) {
    let form = conduit_form::parse_with_startup(
        SIGNAL_DEMO_FORM,
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("R1 Signal form must check against conduit-signal profile");
    for (stem, routes) in [
        (
            "r1_plan_a_signal",
            conduit_r1_network_conformance::R1SignalRouteSet::WebSocketOnly,
        ),
        (
            "r1_plan_b_signal",
            conduit_r1_network_conformance::R1SignalRouteSet::UsbOnly,
        ),
        (
            "r1_plan_c_signal",
            conduit_r1_network_conformance::R1SignalRouteSet::WebSocketThenUsb,
        ),
    ] {
        let exact = conduit_r1_network_conformance::exact_r1_signal_plan(
            BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
            routes,
        )
        .expect("exact R1 recovery Signal Plan must resolve");
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id.as_str() == conduit_r1_network_conformance::R1_PICO_HOST_ID
            })
            .expect("R1 recovery Plan must contain the Pico fragment");
        let lowered = lower_plan_fragment(fragment).expect("R1 Pico Signal fragment must lower");
        let generated = generate_embedded_plan(fragment, &lowered, pico_signal_bounds())
            .expect("R1 Pico Signal fragment must fit reviewed fixed-image bounds");
        let identity = GeneratedFirmwareIdentity::new(&form, &generated);
        let rendered = render_firmware_module(&generated, &identity);
        fs::write(out.join(format!("{stem}_image.rs")), &rendered)
            .expect("generated R1 Pico Signal image should be writable");
        if matches!(
            routes,
            conduit_r1_network_conformance::R1SignalRouteSet::WebSocketOnly
        ) {
            fs::write(out.join("pico_signal_image.rs"), &rendered)
                .expect("active R1 WebSocket Signal image should be writable");
        }
        fs::write(
            out.join(format!("{stem}_identity.json")),
            render_signal_identity_sidecar(&generated, &identity),
        )
        .expect("generated R1 Pico Signal identity sidecar should be writable");
    }
}

fn generate_pico_network_image(out: &Path) {
    let exact = conduit_r1_network_conformance::exact_r1_network_bootstrap_plan()
        .expect("exact R1 USB network bootstrap Plan must resolve");
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment.host_id.as_str() == conduit_r1_network_conformance::R1_PICO_HOST_ID
        })
        .expect("R1 bootstrap Plan must contain the Pico fragment");
    let lowered = lower_plan_fragment(fragment).expect("Pico network fragment must lower");
    let generated = generate_embedded_plan(
        fragment,
        &lowered,
        EmbeddedImageBounds {
            maximum_nodes: 2,
            maximum_cords: 2,
            maximum_routes: 1,
            maximum_route_targets: 1,
            maximum_host_operations: 2,
            maximum_resources: 1,
            maximum_sign_expectations: 8,
            maximum_configuration_entries: 0,
            maximum_ports_per_node:
                conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            maximum_remote_endpoints: 1,
            maximum_cord_value_slots: 2,
            maximum_cord_value_bytes: conduit_net::MAXIMUM_JOIN_INPUT_BYTES
                + conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES,
            maximum_sign_items: 32,
            maximum_sign_bytes: 1024,
        },
    )
    .expect("Pico network fragment must fit reviewed fixed-image bounds");
    let plan_id = PlanId::from(generated.plan_id.clone());
    let host_id = HostId::from(generated.host_id.clone());
    let boot_id = BootId::from(generated.boot_id.clone());
    let active_play = bind_active_play(&plan_id, &host_id, &boot_id, 0);
    let boot_sign = bind_sign(&host_id, &boot_id, None, 0);
    let attachment_sign = bind_sign(&host_id, &boot_id, Some(&active_play.active_play_id), 0);
    let firmware_build_id = format!(
        "conduit-pico-w-signal:{}:{}:{}:{}:{}:{}:{}",
        git_revision(),
        git_tree_state(),
        env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_owned()),
        env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_owned()),
        firmware_mode(),
        generated.plan_id,
        generated.fragment_id,
    );
    let identity = GeneratedFirmwareIdentity {
        firmware_mode: firmware_mode(),
        firmware_build_id: firmware_build_id.clone(),
        source_document_id: fragment.source_document_id.as_str().to_owned(),
        checked_form_id: fragment.checked_form_id.as_str().to_owned(),
        expanded_form_id: fragment.expanded_form_id.as_str().to_owned(),
        active_play_id: active_play.active_play_id.as_str().to_owned(),
        boot_sign_id: boot_sign.sign_id.as_str().to_owned(),
        presentation_ids: Vec::new(),
        presentation_sign_ids: Vec::new(),
        terminal_sign_id: attachment_sign.sign_id.as_str().to_owned(),
    };
    let mut module = generated.render_no_alloc_firmware_module();
    render_string_constant(
        &mut module,
        "SOURCE_DOCUMENT_ID",
        fragment.source_document_id.as_str(),
    );
    render_string_constant(
        &mut module,
        "CHECKED_FORM_ID",
        fragment.checked_form_id.as_str(),
    );
    render_string_constant(
        &mut module,
        "EXPANDED_FORM_ID",
        fragment.expanded_form_id.as_str(),
    );
    render_string_constant(
        &mut module,
        "ACTIVE_PLAY_ID",
        active_play.active_play_id.as_str(),
    );
    render_string_constant(&mut module, "BOOT_SIGN_ID", boot_sign.sign_id.as_str());
    render_string_constant(
        &mut module,
        "ATTACHMENT_SIGN_ID",
        attachment_sign.sign_id.as_str(),
    );
    render_string_constant(&mut module, "FIRMWARE_BUILD_ID", &firmware_build_id);
    fs::write(out.join("pico_network_image.rs"), module)
        .expect("generated Pico network image should be writable");
    let sidecar = render_network_identity_sidecar(&generated, &identity);
    fs::write(out.join("pico_network_identity.json"), &sidecar)
        .expect("generated Pico network identity sidecar should be writable");
    if let Ok(path) = env::var(IDENTITY_SIDECAR_ENV) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .expect("explicit generated Pico network identity directory should be writable");
        }
        fs::write(path, &sidecar)
            .expect("explicit generated Pico network identity sidecar should be writable");
    }
}

fn generate_pico_signal_image(out: &Path) {
    let source = if firmware_mode() == "triple-remote" {
        TRIPLE_SIGNAL_FORM
    } else {
        SIGNAL_DEMO_FORM
    };
    let form = conduit_form::parse_with_startup(
        source,
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("selected Signal form must check against conduit-signal profile");
    let (plan, target_host) = if firmware_mode() == "triple-remote" {
        let exact = triple::exact_plan().expect("exact three-host Signal plan must resolve");
        (exact.plan, triple::PICO_HOST_ID)
    } else if firmware_mode() == "usb-remote" {
        let exact = exact_std_pico_usb_plan().expect("exact std-to-Pico UsbCdc plan must resolve");
        (exact.plan, STD_PICO_USB_SINK_HOST_ID)
    } else if firmware_mode() == "bluetooth-line" {
        // The physical address is mutable observation truth and deliberately
        // excluded from Plan identity; this build-time sentinel is replaced by
        // exact current observation before the Line is offered at runtime.
        let exact = exact_std_pico_bluetooth_plan([0; 6])
            .expect("exact std-to-Pico Bluetooth plan must resolve");
        (exact.plan, STD_PICO_USB_SINK_HOST_ID)
    } else {
        let advertisement = pico_local_advertisement();
        let placements =
            conduit_planner::default_placements(&form, std::slice::from_ref(&advertisement))
                .expect("Pico local advertisement must cover the Signal form");
        let plan = conduit_planner::plan_with_connection_limits(
            &form,
            std::slice::from_ref(&advertisement),
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            SIGNAL_ENCODED_LEN,
        )
        .expect("Pico local Signal form must plan");
        (plan, PICO_LOCAL_HOST_ID)
    };
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == target_host)
        .expect("selected plan must contain one Pico fragment");
    let lowered = lower_plan_fragment(fragment).expect("Pico fragment must lower");
    let generated = generate_embedded_plan(fragment, &lowered, pico_signal_bounds())
        .expect("Pico fragment must fit the reviewed fixed-image bounds");
    let identity = GeneratedFirmwareIdentity::new(&form, &generated);

    fs::write(
        out.join("pico_signal_image.rs"),
        render_firmware_module(&generated, &identity),
    )
    .expect("generated Pico Signal image should be writable");
    let sidecar = render_signal_identity_sidecar(&generated, &identity);
    fs::write(out.join("pico_signal_identity.json"), &sidecar)
        .expect("generated Pico Signal identity sidecar should be writable");
    if let Ok(path) = env::var(IDENTITY_SIDECAR_ENV) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .expect("explicit generated Pico Signal identity directory should be writable");
        }
        fs::write(path, &sidecar)
            .expect("explicit generated Pico Signal identity sidecar should be writable");
    }
}

fn render_firmware_module(
    generated: &GeneratedEmbeddedPlan,
    identity: &GeneratedFirmwareIdentity,
) -> String {
    let mut module = generated.render_no_alloc_firmware_module();
    render_identity_constants(&mut module, identity);
    module.push_str(&format!(
        concat!(
            "pub const MAX_STORED_SIGNAL_VALUES: usize = {};\n",
            "pub const WAIT_VALUE_BYTES: u32 = {};\n",
            "pub const RUNTIME_SIGN_EVENTS: usize = {};\n",
        ),
        MAX_STORED_SIGNAL_VALUES, WAIT_VALUE_BYTES, RUNTIME_SIGN_EVENTS,
    ));
    module
}

struct GeneratedFirmwareIdentity {
    firmware_mode: &'static str,
    firmware_build_id: String,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    active_play_id: String,
    boot_sign_id: String,
    presentation_ids: Vec<String>,
    presentation_sign_ids: Vec<String>,
    terminal_sign_id: String,
}

impl GeneratedFirmwareIdentity {
    fn new(form: &conduit_form::CheckedForm, generated: &GeneratedEmbeddedPlan) -> Self {
        let plan_id = PlanId::from(generated.plan_id.clone());
        let host_id = HostId::from(generated.host_id.clone());
        let boot_id = BootId::from(generated.boot_id.clone());
        let active_play = bind_active_play(&plan_id, &host_id, &boot_id, 0);
        let show_placement = show_placement_id(generated);

        let presentation_ids = (0..MAX_STORED_SIGNAL_VALUES as u64)
            .map(|sequence| {
                bind_presentation(&active_play.active_play_id, &show_placement, sequence)
                    .presentation_id
                    .as_str()
                    .to_owned()
            })
            .collect();
        let presentation_sign_ids = (0..MAX_STORED_SIGNAL_VALUES as u64)
            .map(|sequence| {
                bind_sign(
                    &host_id,
                    &boot_id,
                    Some(&active_play.active_play_id),
                    sequence,
                )
                .sign_id
                .as_str()
                .to_owned()
            })
            .collect();
        let terminal_sign_id = bind_sign(
            &host_id,
            &boot_id,
            Some(&active_play.active_play_id),
            MAX_STORED_SIGNAL_VALUES as u64,
        )
        .sign_id
        .as_str()
        .to_owned();
        let boot_sign_id = bind_sign(&host_id, &boot_id, None, 0)
            .sign_id
            .as_str()
            .to_owned();
        let firmware_build_id = firmware_build_id(form, generated, &active_play.active_play_id);

        Self {
            firmware_mode: firmware_mode(),
            firmware_build_id,
            source_document_id: form.source_document_id.as_str().to_owned(),
            checked_form_id: form.checked_form_id.as_str().to_owned(),
            expanded_form_id: form.expanded_form_id.as_str().to_owned(),
            active_play_id: active_play.active_play_id.as_str().to_owned(),
            boot_sign_id,
            presentation_ids,
            presentation_sign_ids,
            terminal_sign_id,
        }
    }
}

fn firmware_build_id(
    form: &conduit_form::CheckedForm,
    generated: &GeneratedEmbeddedPlan,
    active_play_id: &conduit_core::ActivePlayId,
) -> String {
    format!(
        "conduit-pico-w-signal:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        git_revision(),
        git_tree_state(),
        env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_owned()),
        env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_owned()),
        firmware_mode(),
        form.source_document_id.as_str(),
        form.checked_form_id.as_str(),
        form.expanded_form_id.as_str(),
        generated.plan_id,
        generated.fragment_id,
        active_play_id.as_str(),
    )
}

fn git_revision() -> String {
    command_stdout(["rev-parse", "--verify", "HEAD"]).unwrap_or_else(|| "unknown-revision".into())
}

fn git_tree_state() -> &'static str {
    if std::process::Command::new("git")
        .args(["diff", "--quiet", "--ignore-submodules", "--"])
        .status()
        .is_ok_and(|status| status.success())
    {
        "clean"
    } else {
        "dirty"
    }
}

fn command_stdout<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn show_placement_id(generated: &GeneratedEmbeddedPlan) -> PlacementId {
    let node = generated
        .nodes
        .iter()
        .find(|node| node.kind_id == SHOW_KIND)
        .expect("generated Signal image must contain a Show placement");
    PlacementId::from(node.placement_id.clone())
}

fn render_identity_constants(module: &mut String, identity: &GeneratedFirmwareIdentity) {
    render_string_constant(module, "FIRMWARE_BUILD_ID", &identity.firmware_build_id);
    render_string_constant(module, "SOURCE_DOCUMENT_ID", &identity.source_document_id);
    render_string_constant(module, "CHECKED_FORM_ID", &identity.checked_form_id);
    render_string_constant(module, "EXPANDED_FORM_ID", &identity.expanded_form_id);
    render_string_constant(module, "ACTIVE_PLAY_ID", &identity.active_play_id);
    render_string_constant(module, "BOOT_SIGN_ID", &identity.boot_sign_id);
    render_string_constant(module, "TERMINAL_SIGN_ID", &identity.terminal_sign_id);
    render_string_array(module, "PRESENTATION_IDS", &identity.presentation_ids);
    render_string_array(
        module,
        "PRESENTATION_SIGN_IDS",
        &identity.presentation_sign_ids,
    );
}

fn render_string_constant(module: &mut String, name: &str, value: &str) {
    writeln!(module, "pub const {name}: &str = {value:?};").expect("String writes cannot fail");
}

fn render_string_array(module: &mut String, name: &str, values: &[String]) {
    writeln!(module, "pub const {name}: [&str; {}] = [", values.len())
        .expect("String writes cannot fail");
    for value in values {
        writeln!(module, "    {value:?},").expect("String writes cannot fail");
    }
    module.push_str("];\n");
}

fn pico_signal_bounds() -> EmbeddedImageBounds {
    EmbeddedImageBounds {
        maximum_nodes: 2,
        maximum_cords: 1,
        maximum_routes: 1,
        maximum_route_targets: 1,
        maximum_host_operations: 2,
        maximum_resources: 2,
        maximum_sign_expectations: 8,
        maximum_configuration_entries: 3,
        maximum_ports_per_node:
            conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
        maximum_remote_endpoints: 2,
        maximum_cord_value_slots: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        maximum_cord_value_bytes: SIGNAL_ENCODED_LEN,
        maximum_sign_items: 16,
        maximum_sign_bytes: 1024,
    }
}
