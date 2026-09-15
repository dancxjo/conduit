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
        "CONDUIT_TOUR_SIGN {{\"schema\":\"conduit.conduitos.tour/v2\",\"status\":\"{}\",\"revision\":{},\"chapter\":{},\"stage\":{},\"specimen_id\":\"{}\",\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"source_document_id\":{},\"checked_form_id\":{},\"expanded_form_id\":{},\"plan_id\":{},\"active_play_id\":{},\"result\":{},\"terminal\":{},\"manifestations\":{},\"comparison_expanded_form_id\":{},\"comparison_plan_id\":{},\"source_fragment_id\":{},\"sink_fragment_id\":{},\"source_active_play_id\":{},\"sink_active_play_id\":{},\"line_id\":{},\"transferred_values\":{},\"workspace_surface_id\":\"{}\",\"workspace_presentation_id\":\"{}\",\"workspace_manifestation_id\":\"{}\",\"status_surface_id\":\"{}\",\"status_presentation_id\":\"{}\",\"status_manifestation_id\":\"{}\",\"frame_sequence\":{},\"surfaces_composed\":{},\"damage_count\":{},\"proof_class\":\"freestanding-emulator\",\"bounded\":true}}\n",
        match state.phase {
            conduit_tour_model::TourWorkspacePhase::LessonReady => "tour-opened",
            conduit_tour_model::TourWorkspacePhase::ResultVisible => "result-visible",
            conduit_tour_model::TourWorkspacePhase::PatchbayOpen => "patchbay-open",
        },
        state.revision,
        state.progress.chapter,
        state.progress.stage,
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
        json_optional(play.map(|value| match value.terminal {
            conduit_tour_model::TourRunTerminal::Completed => "completed",
            conduit_tour_model::TourRunTerminal::Stopped => "stopped",
        })),
        json_optional_number(play.map(|value| u64::from(value.manifestations))),
        json_optional(play.and_then(|value| {
            value
                .comparison_expanded_form_id
                .as_ref()
                .map(|id| id.as_str())
        })),
        json_optional(
            play.and_then(|value| value.comparison_plan_id.as_ref().map(|id| id.as_str()))
        ),
        json_optional(play.and_then(|value| {
            value
                .multi_host
                .as_ref()
                .map(|proof| proof.source_fragment_id.as_str())
        })),
        json_optional(play.and_then(|value| {
            value
                .multi_host
                .as_ref()
                .map(|proof| proof.sink_fragment_id.as_str())
        })),
        json_optional(play.and_then(|value| {
            value
                .multi_host
                .as_ref()
                .map(|proof| proof.source_active_play_id.as_str())
        })),
        json_optional(play.and_then(|value| {
            value
                .multi_host
                .as_ref()
                .map(|proof| proof.sink_active_play_id.as_str())
        })),
        json_optional(play.and_then(|value| {
            value
                .multi_host
                .as_ref()
                .map(|proof| proof.line_id.as_str())
        })),
        json_optional_number(play.and_then(|value| {
            value
                .multi_host
                .as_ref()
                .map(|proof| u64::from(proof.transferred_values))
        })),
        shell.workspace.surface_id,
        shell.workspace.presentation_id.as_str(),
        shell.workspace.manifestation_id.as_str(),
        shell.status.surface_id,
        shell.status.presentation_id.as_str(),
        shell.status.manifestation_id.as_str(),
        shell.frame.frame_sequence,
        shell.frame.surfaces_composed,
        shell.frame.damage_count,
    );
    arch::early_write(line.as_bytes());
}

fn json_optional_number(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |value| format!("{value}"))
}

fn json_optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), |value| format!("\"{value}\""))
}
