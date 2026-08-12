//! Machine-readable retained identity and bound receipt for the mixed-Body capstone.

use conduit_body::{Body, BodyMembership};
use conduit_core::Plan;
use patchbay_model::{PartPresentationState, PartsView};
use serde_json::{json, Value};

pub(super) fn machine_receipt(
    body: &Body,
    membership: &BodyMembership,
    view: &PartsView,
    active_plan: &Plan,
    replacement_plan: &Plan,
    physical_pico: bool,
) -> Result<String, String> {
    let parts = view
        .parts
        .iter()
        .map(|row| {
            json!({
                "part_id": row.details.part_id.as_str(),
                "host_id": row.details.host_id.as_ref().map(|id| id.as_str()),
                "boot_id": row.details.boot_id.as_ref().map(|id| id.as_str()),
                "offer_generation": row.details.offer_generation.map(|generation| generation.0),
                "presentation_state": match row.state {
                    PartPresentationState::Here => "here",
                    PartPresentationState::Attached => "attached",
                    PartPresentationState::Offline => "offline",
                },
                "available": row.available,
                "in_plan": row.in_plan,
                "playing": row.playing,
            })
        })
        .collect::<Vec<Value>>();
    serde_json::to_string(&json!({
        "schema": "conduit.body/mixed-membership-capstone@1",
        "proof_class": if physical_pico { "live-browser-plus-physical-hardware" } else { "live-browser" },
        "body_id": body.body_id.as_str(),
        "source_document_id": active_plan.source_document_id.as_str(),
        "checked_form_id": active_plan.checked_form_id.as_str(),
        "membership_revision": membership.revision.0,
        "membership_event_count": membership.events.len(),
        "parts": parts,
        "active_plan_id": active_plan.plan_id.as_str(),
        "replacement_plan_id": replacement_plan.plan_id.as_str(),
        "active_plan_unchanged_by_join": true,
        "replacement_plan_distinct": active_plan.plan_id != replacement_plan.plan_id,
        "future_realization_possibilities": view.new_realization_possibilities,
        "physical_pico_admitted": physical_pico,
        "browser_parts": 3,
        "declared_bounds": {
            "body_parts": conduit_body::MAX_BODY_PARTS,
            "membership_events": conduit_body::MAX_MEMBERSHIP_EVENTS,
            "candidates": conduit_body::MAX_CANDIDATES,
            "pending_admissions": conduit_body::MAX_PENDING_ADMISSIONS,
            "admission_receipts": conduit_body::MAX_ADMISSION_RECEIPTS,
            "pico_frame_bytes": conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES,
        }
    }))
    .map_err(|error| format!("encode capstone receipt: {error}"))
}
