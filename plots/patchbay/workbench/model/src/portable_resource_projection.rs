//! Resource residence disclosure over sealed Plan facts, separate from Cord/Line.
use crate::portable_projection::ContentBuilder;
use conduit_core::{PlanFragment, PlannedGear};
use conduit_presentation::PresentationPropertyValue;

pub(super) fn append_resources(
    content: &mut ContentBuilder,
    subject: &str,
    placement: &PlannedGear,
    fragments: &[PlanFragment],
) {
    for (index, resource) in placement.resources.iter().enumerate() {
        content.property(
            subject,
            &format!("resource-{index}"),
            PresentationPropertyValue::Text(format!(
                "{} · class {} · units {}",
                resource.pool_id.as_str(),
                resource.class_id.as_str(),
                resource.units
            )),
        );
        let Some(residence) = &resource.content else {
            continue;
        };
        let c = &residence.contract;
        let identity = c
            .identity
            .digest()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let version = c
            .version
            .digest()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        for (key,value) in [
            ("meaning",format!("RESOURCE {} · identity {identity} · generation {version}",c.content_profile.as_str())),
            ("access",format!("{:?} · {:?} · {:?} · sensitive {}",c.access,c.sharing,c.retention,c.sensitive)),
            ("bounds",format!("{} bytes · {} items · {} generations · {} reader leases · {} publication slots",c.maximum_bytes,c.maximum_items,c.generation_slots,c.reader_leases,c.publication_slots)),
            ("residence",format!("owner {} · Boot {} · residence {} · Base {}",residence.owner_host.as_str(),residence.owner_boot.as_str(),residence.residence_profile.as_str(),residence.base_id.as_str())),
        ] { content.property(subject,&format!("resource-{index}-{key}"),PresentationPropertyValue::Text(value)); }
    }
    append_state_boundaries(content, subject, placement, fragments);
}

/// Expose the exact retained-State contract sealed into the Plan without
/// confusing requested survival with proof that recovery has occurred.
pub(super) fn append_state_boundaries(
    content: &mut ContentBuilder,
    subject: &str,
    placement: &PlannedGear,
    fragments: &[PlanFragment],
) {
    for (index, state) in fragments
        .iter()
        .flat_map(|fragment| &fragment.states)
        .filter(|state| state.gear_id == placement.gear_id)
        .enumerate()
    {
        for (key, value) in [
            ("identity", state.state_id.as_str().to_owned()),
            ("value-kind", state.value_kind.as_str().to_owned()),
            ("requested-lifetime", format!("{:?}", state.lifetime)),
            ("maximum-value-bytes", state.maximum_value_bytes.to_string()),
            ("continuation", format!("{:?}", state.continuation)),
            (
                "initialization",
                state.initial_value.as_ref().map_or_else(
                    || "absent".to_owned(),
                    |value| format!("present · {} bytes", value.len()),
                ),
            ),
        ] {
            content.property(
                subject,
                &format!("state-{index}-{key}"),
                PresentationPropertyValue::Text(value),
            );
        }

        let Some(retained) = &state.retained else {
            content.property(
                subject,
                &format!("state-{index}-retained"),
                PresentationPropertyValue::Text(
                    "absent · requested retention only; no recovery claimed".into(),
                ),
            );
            continue;
        };
        let source = &retained.source_play;
        for (key, value) in [
            (
                "retained-source-plot",
                format!(
                    "source {} · checked {} · expanded {}",
                    retained.source_plot.source_document_id.as_str(),
                    retained.source_plot.checked_plot_id.as_str(),
                    retained.source_plot.expanded_plot_id.as_str()
                ),
            ),
            (
                "retained-source-play",
                format!(
                    "active {} · plan {} · sequence {}",
                    source.active_play_id.as_str(),
                    source.plan_id.as_str(),
                    source.play_sequence
                ),
            ),
            (
                "retained-source-host",
                format!(
                    "Host {} · Boot {}",
                    source.host_id.as_str(),
                    source.boot_id.as_str()
                ),
            ),
            (
                "retained-generation",
                format!(
                    "state {} · kind {} · generation {} · {} bytes",
                    retained.source_state.as_str(),
                    retained.value_kind.as_str(),
                    retained.generation,
                    retained.current_value.len()
                ),
            ),
        ] {
            content.property(
                subject,
                &format!("state-{index}-{key}"),
                PresentationPropertyValue::Text(value),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        bind_active_play, seal_plan_with_realization_backs_and_completion, state_resource_budget,
        PlannedStateBoundary, PlotIdentity, RetainedStateProvenance, StateContinuation, StateId,
        StateLifetime,
    };
    use conduit_planner::proof::resource_frame::frame_resource_plan;

    #[test]
    fn retained_state_and_saved_resource_descend_as_distinct_exact_plan_truth() {
        let mut proof = frame_resource_plan(false, false).unwrap();
        let placement = proof.plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.inputs.len() == 1 && placement.outputs.len() == 1)
            .unwrap()
            .clone();
        let state_id = StateId::from("state/frame-compose/current");
        let source_play = bind_active_play(
            &conduit_core::PlanId::from("plan/frame-compose/prior"),
            &placement.host_id,
            &placement.boot_id,
            3,
        );
        let state = PlannedStateBoundary {
            state_id: state_id.clone(),
            gear_id: placement.gear_id.clone(),
            value_kind: placement.inputs[0].value_kind.clone(),
            initial_value: Some(vec![1, 2]),
            lifetime: StateLifetime::Body,
            retained: Some(RetainedStateProvenance {
                source_plot: PlotIdentity {
                    source_document_id: proof.plan.source_document_id.clone(),
                    checked_plot_id: proof.plan.checked_plot_id.clone(),
                    expanded_plot_id: proof.plan.expanded_plot_id.clone(),
                },
                source_play,
                source_state: state_id,
                value_kind: placement.inputs[0].value_kind.clone(),
                generation: 4,
                current_value: vec![7, 8, 9],
            }),
            maximum_value_bytes: 512,
            continuation: StateContinuation::MaximumTransitions(9),
        };
        let budget = state_resource_budget(core::slice::from_ref(&state)).unwrap();
        let mut fragments = proof.plan.fragments.clone();
        fragments[0].states.push(state);
        fragments[0].sign_storage_budget.item_capacity += budget.sign_storage.item_capacity;
        fragments[0].sign_storage_budget.byte_capacity += budget.sign_storage.byte_capacity;
        proof.plan = seal_plan_with_realization_backs_and_completion(
            PlotIdentity {
                source_document_id: proof.plan.source_document_id.clone(),
                checked_plot_id: proof.plan.checked_plot_id.clone(),
                expanded_plot_id: proof.plan.expanded_plot_id.clone(),
            },
            proof.plan.completion_policy,
            proof.plan.realization_backs.clone(),
            fragments,
        );
        assert!(conduit_core::verify_plan(&proof.plan));
        let before = serde_json::to_vec(&proof.plan).unwrap();
        let graph = crate::PatchbayGraph::from_expanded(&proof.expanded).unwrap();
        let document = crate::PlanDocument::from_plan(
            crate::PatchbayRequestId::new("resource-inspect").unwrap(),
            &proof.plan,
        )
        .unwrap();
        let mut content = ContentBuilder::new();
        let plot = content.subject_with_identity(
            "plot/frames",
            conduit_presentation::PresentationRole::Plot,
            "Frame Resource proof",
        );
        crate::portable_graph_projection::append_exact_graph(
            &plot,
            &graph,
            Some(&document),
            None,
            &mut content,
        );
        let body = conduit_body::Body::born(
            proof.plan.source_document_id.clone(),
            proof.plan.checked_plot_id.clone(),
            1,
            "sign/frame-born".into(),
        )
        .unwrap();
        let presentation = conduit_presentation::Presentation::new(
            1,
            conduit_presentation::PresentationBasis {
                body_id: Some(body.body_id),
                wake_id: None,
                source_document_id: Some(proof.plan.source_document_id.clone()),
                checked_plot_id: Some(proof.plan.checked_plot_id.clone()),
                expanded_plot_id: Some(proof.plan.expanded_plot_id.clone()),
                plan_id: Some(proof.plan.plan_id.clone()),
                active_play_id: None,
                sign_ids: vec![],
            },
            content.subjects,
            content.relationships,
            content.properties,
            content.text,
        )
        .unwrap();
        let projected = format!("{:?}", presentation.properties);
        for expected in [
            "RESOURCE image/rgba@1",
            "generation",
            "ReadPublished",
            "WriteCandidatePublish",
            "Play",
            "SingleWriterPublished",
            "arena/shared-read@1",
            "reader leases",
            "publication slots",
            "state/frame-compose/current",
            "Body",
            "MaximumTransitions(9)",
            "present · 2 bytes",
            "plan/frame-compose/prior",
            "generation 4 · 3 bytes",
            "resource-",
            "state-",
            "Line",
        ] {
            assert!(
                projected.contains(expected),
                "missing {expected}: {projected}"
            );
        }
        assert!(!projected.contains("no recovery claimed"));
        assert_eq!(serde_json::to_vec(&proof.plan).unwrap(), before);

        let mut requested_only = proof.plan.fragments.clone();
        requested_only[0].states[0].retained = None;
        let mut requested_content = ContentBuilder::new();
        let requested_subject = requested_content.subject_with_identity(
            "gear/requested-retention",
            conduit_presentation::PresentationRole::Gear,
            "Requested retention",
        );
        append_state_boundaries(
            &mut requested_content,
            &requested_subject,
            &placement,
            &requested_only,
        );
        let requested = format!("{:?}", requested_content.properties);
        assert!(requested.contains("requested retention only; no recovery claimed"));
        assert!(!requested.contains("retained-source-play"));
    }
}
