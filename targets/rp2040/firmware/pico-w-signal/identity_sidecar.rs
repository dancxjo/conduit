use std::fmt::Write as _;

use conduit_embedded_build::GeneratedEmbeddedPlan;

use super::GeneratedFirmwareIdentity;

pub(super) fn render_network_identity_sidecar(
    generated: &GeneratedEmbeddedPlan,
    identity: &GeneratedFirmwareIdentity,
) -> String {
    render_identity_sidecar(
        generated,
        identity,
        "conduit.pico-network.generated-image@1",
    )
}

pub(super) fn render_signal_identity_sidecar(
    generated: &GeneratedEmbeddedPlan,
    identity: &GeneratedFirmwareIdentity,
) -> String {
    render_identity_sidecar(generated, identity, "conduit.pico-signal.generated-image@1")
}

fn render_identity_sidecar(
    generated: &GeneratedEmbeddedPlan,
    identity: &GeneratedFirmwareIdentity,
    schema: &str,
) -> String {
    let presentation_ids = json_string_array(&identity.presentation_ids);
    let presentation_sign_ids = json_string_array(&identity.presentation_sign_ids);
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"{}\",\n",
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
            "  \"boot_sign_id\": \"{}\",\n",
            "  \"presentation_ids\": {},\n",
            "  \"presentation_sign_ids\": {},\n",
            "  \"terminal_sign_id\": \"{}\",\n",
            "  \"offer_generation\": {},\n",
            "  \"nodes\": {},\n",
            "  \"cords\": {},\n",
            "  \"host_operations\": {},\n",
            "  \"cord_value_slots\": {},\n",
            "  \"cord_value_bytes\": {},\n",
            "  \"sign_items\": {},\n",
            "  \"sign_bytes\": {}\n",
            "}}\n"
        ),
        schema,
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
        json_escape(&identity.boot_sign_id),
        presentation_ids,
        presentation_sign_ids,
        json_escape(&identity.terminal_sign_id),
        generated.offer_generation,
        generated.nodes.len(),
        generated.cords.len(),
        generated.host_operations.len(),
        generated.cord_value_slots,
        generated.cord_value_bytes,
        generated.sign_items,
        generated.sign_bytes,
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
