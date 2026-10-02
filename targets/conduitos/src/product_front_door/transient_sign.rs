//! Durable evidence for transient Presentation surface lifecycle.

use alloc::format;

use crate::{
    arch,
    identity::{self, BootIdentities},
    make::MakeRecord,
    tour_shell::ShellTransientDismissalReceipt,
};

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
