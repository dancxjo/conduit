//! Exact live evidence for the retained multi-surface Tour shell.

use alloc::{format, string::String};

use crate::{
    arch,
    fabrication::FabricationRecord,
    identity::{self, BootIdentities},
    tour_product::{TourProduct, TourProductUpdate},
    tour_shell::ShellPresentationReceipt,
};

pub(super) fn emit_tour_sign(
    tour: &TourProduct,
    update: Option<&TourProductUpdate>,
    shell: &ShellPresentationReceipt,
    identities: &BootIdentities,
    fabrication: &FabricationRecord,
) {
    let state = tour.controller().state();
    let play = update.and_then(|value| value.play.as_ref());
    let line = format!(
        "CONDUIT_TOUR_SIGN {{\"schema\":\"conduit.conduitos.tour/v1\",\"status\":\"{}\",\"revision\":{},\"specimen_id\":\"{}\",\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"source_document_id\":{},\"checked_form_id\":{},\"expanded_form_id\":{},\"plan_id\":{},\"active_play_id\":{},\"result\":{},\"workspace_surface_id\":\"{}\",\"workspace_presentation_id\":\"{}\",\"workspace_manifestation_id\":\"{}\",\"status_surface_id\":\"{}\",\"status_presentation_id\":\"{}\",\"status_manifestation_id\":\"{}\",\"frame_sequence\":{},\"surfaces_composed\":{},\"damage_count\":{},\"proof_class\":\"freestanding-emulator\",\"bounded\":true}}\n",
        match state.phase {
            conduit_tour_model::TourWorkspacePhase::LessonReady => "tour-opened",
            conduit_tour_model::TourWorkspacePhase::ResultVisible => "result-visible",
            conduit_tour_model::TourWorkspacePhase::PatchbayOpen => "patchbay-open",
        },
        state.revision,
        state.specimen_id,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
        json_optional(play.map(|value| value.source_document_id.as_str())),
        json_optional(play.map(|value| value.checked_form_id.as_str())),
        json_optional(play.map(|value| value.expanded_form_id.as_str())),
        json_optional(play.map(|value| value.plan_id.as_str())),
        json_optional(play.map(|value| value.active_play_id.as_str())),
        json_optional(state.result.as_deref()),
        shell.workspace.surface_id,
        shell.workspace.presentation_id,
        shell.workspace.manifestation_id,
        shell.status.surface_id,
        shell.status.presentation_id,
        shell.status.manifestation_id,
        shell.frame.frame_sequence,
        shell.frame.surfaces_composed,
        shell.frame.damage_count,
    );
    arch::early_write(line.as_bytes());
}

fn json_optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), |value| format!("\"{value}\""))
}
