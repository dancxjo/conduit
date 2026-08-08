use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use conduit_core::{
    bind_active_play, bind_evidence, bind_presentation, BootId, ConnectionProvider, HostId,
    PlacementId, PlanId,
};
use conduit_embedded_build::{
    generate_embedded_plan, EmbeddedImageBounds, GeneratedEmbeddedPlan,
};
use conduit_runtime::lowering::lower_plan_fragment;
use conduit_signal::{
    exact_std_pico_usb_plan, pico_local_advertisement, signal_profile_catalog,
    triple,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, PICO_LOCAL_HOST_ID, SHOW_KIND, SIGNAL_ENCODED_LEN,
    STD_PICO_USB_SINK_HOST_ID,
};

const SIGNAL_DEMO_FORM: &str = include_str!("../../examples/signal-demo.form");
const TRIPLE_SIGNAL_FORM: &str = include_str!("../../examples/triple-signal.form");
const IDENTITY_SIDECAR_ENV: &str = "CONDUIT_PICO_SIGNAL_IDENTITY_SIDECAR";
const IDENTITY_SIDECAR_RERUN_ENV: &str = "CONDUIT_PICO_SIGNAL_IDENTITY_RERUN";
const MAX_STORED_SIGNAL_VALUES: usize = 16;
const WAIT_VALUE_BYTES: u32 = 8;
const RUNTIME_EVIDENCE_EVENTS: usize = 256;

fn main() {
    let target = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    generate_pico_signal_image(&out);

    // Only emit the RP2040-specific linker flags for thumbv6m
    if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "arm"
        && env::var("TARGET").unwrap_or_default() == "thumbv6m-none-eabi"
    {
        let memory_x = include_str!("memory.x");
        fs::write(out.join("memory.x"), memory_x).unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
    let _ = target;
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../examples/signal-demo.form");
    println!("cargo:rerun-if-changed=../../examples/triple-signal.form");
    println!("cargo:rerun-if-env-changed={IDENTITY_SIDECAR_ENV}");
    println!("cargo:rerun-if-env-changed={IDENTITY_SIDECAR_RERUN_ENV}");
}

fn generate_pico_signal_image(out: &Path) {
    let source = if firmware_mode() == "triple-remote" {
        TRIPLE_SIGNAL_FORM
    } else {
        SIGNAL_DEMO_FORM
    };
    let form = conduit_form::parse(source, &signal_profile_catalog())
        .expect("selected Signal form must check against conduit-signal profile");
    let (plan, target_host) = if firmware_mode() == "triple-remote" {
        let exact = triple::exact_plan().expect("exact three-host Signal plan must resolve");
        (exact.plan, triple::PICO_HOST_ID)
    } else if firmware_mode() == "usb-remote" {
        let exact = exact_std_pico_usb_plan().expect("exact std-to-Pico UsbCdc plan must resolve");
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
            &[ConnectionProvider::Local],
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
    let identity = GeneratedSignalIdentity::new(&form, &generated);

    fs::write(
        out.join("pico_signal_image.rs"),
        render_firmware_module(&generated, &identity),
    )
    .expect("generated Pico Signal image should be writable");
    let sidecar = render_identity_sidecar(&generated, &identity);
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
    identity: &GeneratedSignalIdentity,
) -> String {
    let mut module = generated.render_no_alloc_firmware_module();
    render_identity_constants(&mut module, identity);
    module.push_str(&format!(
        concat!(
            "pub const MAX_STORED_SIGNAL_VALUES: usize = {};\n",
            "pub const WAIT_VALUE_BYTES: u32 = {};\n",
            "pub const RUNTIME_EVIDENCE_EVENTS: usize = {};\n",
        ),
        MAX_STORED_SIGNAL_VALUES,
        WAIT_VALUE_BYTES,
        RUNTIME_EVIDENCE_EVENTS,
    ));
    module
}

struct GeneratedSignalIdentity {
    firmware_mode: &'static str,
    firmware_build_id: String,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    active_play_id: String,
    boot_evidence_id: String,
    presentation_ids: Vec<String>,
    presentation_evidence_ids: Vec<String>,
    terminal_evidence_id: String,
}

impl GeneratedSignalIdentity {
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
        let presentation_evidence_ids = (0..MAX_STORED_SIGNAL_VALUES as u64)
            .map(|sequence| {
                bind_evidence(
                    &host_id,
                    &boot_id,
                    Some(&active_play.active_play_id),
                    sequence,
                )
                .evidence_id
                .as_str()
                .to_owned()
            })
            .collect();
        let terminal_evidence_id = bind_evidence(
            &host_id,
            &boot_id,
            Some(&active_play.active_play_id),
            MAX_STORED_SIGNAL_VALUES as u64,
        )
        .evidence_id
        .as_str()
        .to_owned();
        let boot_evidence_id = bind_evidence(&host_id, &boot_id, None, 0)
            .evidence_id
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
            boot_evidence_id,
            presentation_ids,
            presentation_evidence_ids,
            terminal_evidence_id,
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

fn firmware_mode() -> &'static str {
    if env::var_os("CARGO_FEATURE_TRIPLE_REMOTE").is_some() {
        "triple-remote"
    } else if env::var_os("CARGO_FEATURE_USB_REMOTE").is_some() {
        "usb-remote"
    } else if env::var_os("CARGO_FEATURE_PICO_LOCAL_MINIMAL").is_some() {
        "pico-local-minimal"
    } else {
        "pico-local"
    }
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

fn render_identity_constants(module: &mut String, identity: &GeneratedSignalIdentity) {
    render_string_constant(module, "FIRMWARE_BUILD_ID", &identity.firmware_build_id);
    render_string_constant(module, "SOURCE_DOCUMENT_ID", &identity.source_document_id);
    render_string_constant(module, "CHECKED_FORM_ID", &identity.checked_form_id);
    render_string_constant(module, "EXPANDED_FORM_ID", &identity.expanded_form_id);
    render_string_constant(module, "ACTIVE_PLAY_ID", &identity.active_play_id);
    render_string_constant(module, "BOOT_EVIDENCE_ID", &identity.boot_evidence_id);
    render_string_constant(
        module,
        "TERMINAL_EVIDENCE_ID",
        &identity.terminal_evidence_id,
    );
    render_string_array(module, "PRESENTATION_IDS", &identity.presentation_ids);
    render_string_array(
        module,
        "PRESENTATION_EVIDENCE_IDS",
        &identity.presentation_evidence_ids,
    );
}

fn render_string_constant(module: &mut String, name: &str, value: &str) {
    writeln!(module, "pub const {name}: &str = {value:?};")
        .expect("String writes cannot fail");
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
        maximum_evidence_expectations: 8,
        maximum_configuration_entries: 3,
        maximum_ports_per_node: conduit_runtime::lowering::MAXIMUM_KERNEL_PORTS_PER_NODE,
        maximum_remote_endpoints: 1,
        maximum_cord_value_slots: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        maximum_cord_value_bytes: SIGNAL_ENCODED_LEN,
        maximum_evidence_items: 16,
        maximum_evidence_bytes: 1024,
    }
}

fn render_identity_sidecar(
    generated: &GeneratedEmbeddedPlan,
    identity: &GeneratedSignalIdentity,
) -> String {
    let presentation_ids = json_string_array(&identity.presentation_ids);
    let presentation_evidence_ids = json_string_array(&identity.presentation_evidence_ids);
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"conduit.pico-signal.generated-image@1\",\n",
            "  \"firmware_mode\": \"{}\",\n",
            "  \"firmware_build_id\": \"{}\",\n",
            "  \"source_document_id\": \"{}\",\n",
            "  \"checked_form_id\": \"{}\",\n",
            "  \"expanded_form_id\": \"{}\",\n",
            "  \"plan_id\": \"{}\",\n",
            "  \"fragment_id\": \"{}\",\n",
            "  \"host_id\": \"{}\",\n",
            "  \"boot_id\": \"{}\",\n",
            "  \"active_play_id\": \"{}\",\n",
            "  \"boot_evidence_id\": \"{}\",\n",
            "  \"presentation_ids\": {},\n",
            "  \"presentation_evidence_ids\": {},\n",
            "  \"terminal_evidence_id\": \"{}\",\n",
            "  \"offer_generation\": {},\n",
            "  \"nodes\": {},\n",
            "  \"cords\": {},\n",
            "  \"host_operations\": {},\n",
            "  \"cord_value_slots\": {},\n",
            "  \"cord_value_bytes\": {},\n",
            "  \"evidence_items\": {},\n",
            "  \"evidence_bytes\": {}\n",
            "}}\n"
        ),
        identity.firmware_mode,
        json_escape(&identity.firmware_build_id),
        json_escape(&identity.source_document_id),
        json_escape(&identity.checked_form_id),
        json_escape(&identity.expanded_form_id),
        json_escape(&generated.plan_id),
        json_escape(&generated.fragment_id),
        json_escape(&generated.host_id),
        json_escape(&generated.boot_id),
        json_escape(&identity.active_play_id),
        json_escape(&identity.boot_evidence_id),
        presentation_ids,
        presentation_evidence_ids,
        json_escape(&identity.terminal_evidence_id),
        generated.offer_generation,
        generated.nodes.len(),
        generated.cords.len(),
        generated.host_operations.len(),
        generated.cord_value_slots,
        generated.cord_value_bytes,
        generated.evidence_items,
        generated.evidence_bytes,
    )
}

fn json_string_array(values: &[String]) -> String {
    let mut output = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        write!(output, "\"{}\"", json_escape(value)).expect("String writes cannot fail");
    }
    output.push(']');
    output
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }
    escaped
}
