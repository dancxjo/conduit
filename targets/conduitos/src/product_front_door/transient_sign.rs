//! Durable evidence for transient Presentation surface lifecycle.

use alloc::format;

use crate::{
    arch,
    identity::{self, BootIdentities},
    make::MakeRecord,
    native_compositor::InputRoute,
    tour_shell::{ShellTransientDismissalReceipt, ShellTransientReceipt, TourShellPresenter},
};

pub(crate) fn emit_shown_transient(
    receipt: &ShellTransientReceipt,
    cause: Option<&str>,
    shell: &TourShellPresenter,
    identities: &BootIdentities,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    let InputRoute::Delivered(keyboard) = shell.route_keyboard().map_err(|error| error.as_str())?
    else {
        return Err("transient-keyboard-route-absent");
    };
    let line = format!(
        "CONDUIT_TRANSIENT_SIGN {{\"schema\":\"conduit.conduitos.transient-surface/v1\",\"status\":\"shown\",\"kind\":\"{}\",\"cause\":{},\"surface_id\":\"{}\",\"presentation_id\":\"{}\",\"manifestation_id\":\"{}\",\"parent_presentation_id\":\"{}\",\"parent_manifestation_id\":\"{}\",\"keyboard_surface_id\":\"{}\",\"keyboard_manifestation_id\":\"{}\",\"frame_sequence\":{},\"surfaces_composed\":{},\"damage_count\":{},\"pixels_written\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"bounded\":true}}\n",
        receipt.kind.as_str(),
        json_optional(cause),
        receipt.transient.surface_id,
        receipt.transient.presentation_id.as_str(),
        receipt.transient.manifestation_id.as_str(),
        receipt.parent_presentation_id.as_str(),
        receipt.parent_manifestation_id.as_str(),
        keyboard.surface_id,
        keyboard.manifestation_id.as_str(),
        receipt.frame.frame_sequence,
        receipt.frame.surfaces_composed,
        receipt.frame.damage_count,
        receipt.frame.pixels_written,
        make.profile_id,
        make.build_id,
        make.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
    );
    arch::early_write(line.as_bytes());
    Ok(())
}

pub(crate) fn emit_dismissed_transient(
    receipt: &ShellTransientDismissalReceipt,
    stale_input_refused: bool,
    identities: &BootIdentities,
    make: &MakeRecord,
) {
    let line = format!(
        "CONDUIT_TRANSIENT_SIGN {{\"schema\":\"conduit.conduitos.transient-surface/v1\",\"status\":\"dismissed\",\"surface_id\":\"{}\",\"manifestation_id\":\"{}\",\"frame_sequence\":{},\"surfaces_composed\":{},\"damage_count\":{},\"pixels_written\":{},\"stale_input_refused\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"bounded\":true}}\n",
        receipt.surface_id,
        receipt.manifestation_id.as_str(),
        receipt.frame.frame_sequence,
        receipt.frame.surfaces_composed,
        receipt.frame.damage_count,
        receipt.frame.pixels_written,
        stale_input_refused,
        make.profile_id,
        make.build_id,
        make.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
    );
    arch::early_write(line.as_bytes());
}

fn json_optional(value: Option<&str>) -> alloc::string::String {
    value.map_or_else(|| "null".into(), |value| format!("\"{value}\""))
}
