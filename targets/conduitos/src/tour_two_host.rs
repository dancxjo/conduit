//! Product-level preparation and proof projection for the two-Host Tour stages.

use crate::{
    identity::BootIdentities,
    machine::SerialBase,
    offer::HostOffer,
    ordinary_plan::PreparationError,
    tour_play::{TourPlayError, TourPlayEvidence},
};

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<crate::tour_two_host_plan::PreparedTourTwoHostPlan, PreparationError> {
    crate::tour_two_host_plan::prepare(identities, offer, build_id)
}

pub fn run(
    prepared: &mut crate::tour_two_host_plan::PreparedTourTwoHostPlan,
    serial: &mut impl SerialBase,
) -> Result<TourPlayEvidence, TourPlayError> {
    let run = crate::tour_two_host_play::run(prepared, serial).map_err(TourPlayError::Machine)?;
    Ok(TourPlayEvidence {
        specimen_id: "canonical-form:hello-across",
        source_document_id: prepared.plan.source_document_id.clone(),
        checked_form_id: prepared.plan.checked_form_id.clone(),
        expanded_form_id: prepared.plan.expanded_form_id.clone(),
        plan_id: prepared.plan.plan_id.clone(),
        active_play_id: prepared.source_active.active_play_id.clone(),
        result: "hello across one cord",
        manifestations: 1,
        comparison_expanded_form_id: None,
        comparison_plan_id: None,
        multi_host: Some(conduit_tour_model::TourMultiHostProof {
            source_fragment_id: prepared.source_fragment_id.as_str().into(),
            sink_fragment_id: prepared.sink_fragment_id.as_str().into(),
            source_active_play_id: prepared.source_active.active_play_id.as_str().into(),
            sink_active_play_id: prepared.sink_active.active_play_id.as_str().into(),
            line_id: prepared.line_id.as_str().into(),
            transferred_values: 1,
        }),
        terminal: conduit_tour_model::TourRunTerminal::Completed,
        run,
        observations: crate::text_composition::TextObservations::default(),
    })
}
