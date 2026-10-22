//! Initial source-owned observation without an event or a fabricated interaction.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::BodyId;
use conduit_face::{
    FaceContributionBasis, FaceDisclosure, FaceDisclosureLevel, FaceFragment, FaceFragmentError,
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};

use super::{PatchbayActivePlotState, PatchbayApplicationPort, graph_face};

impl PatchbayApplicationPort {
    /// Prepare read-only resident meaning before any input. The caller supplies
    /// observed owner context; this value grants no action or return authority.
    /// The outer Face owns Body/resident Plot declarations and final validation.
    /// Production Fore/Show/return-route migration is a separate admission step.
    pub fn observe_fragment(
        &self,
        basis: FaceContributionBasis,
        body: &BodyId,
    ) -> Result<FaceFragment, FaceFragmentError> {
        let root = format!("patchbay/document/{}", basis.checked_plot_id.as_str());
        let body = format!("body/{}", body.as_str());
        let mut fragment = FaceFragment {
            basis,
            subjects: vec![FaceSubject {
                identity: root.clone(),
                role: FaceRole::Document,
                name: "Patchbay".into(),
            }],
            relationships: vec![FaceRelationship {
                source: root.clone(),
                target: body.clone(),
                kind: FaceRelationshipKind::Observes,
            }],
            composition: Vec::new(),
            properties: vec![
                property(
                    &root,
                    "application-revision",
                    FacePropertyValue::Count(u64::from(self.revision)),
                ),
                property(
                    &root,
                    "body-plan-id",
                    FacePropertyValue::Identity(self.body_plan_id.as_str().into()),
                ),
            ],
            text: vec![FaceText {
                subject: root.clone(),
                text: "Active plots on this body".into(),
            }],
            // Merely observing meaning is not an admitted effect return route.
            actions: Vec::new(),
            disclosures: vec![FaceDisclosure {
                subject: root.clone(),
                level: FaceDisclosureLevel::Primary,
            }],
            temporal_references: Vec::new(),
            temporal_facts: Vec::new(),
        };
        if let Some(play) = &self.active_play_id {
            fragment.properties.push(property(
                &root,
                "observed-body-play-id",
                FacePropertyValue::Identity(play.as_str().into()),
            ));
        }
        for (index, plot) in self.plots.iter().enumerate() {
            let identity = format!("plot/{}", plot.graph.checked_plot_id.as_str());
            fragment.relationships.push(FaceRelationship {
                source: root.clone(),
                target: identity.clone(),
                kind: FaceRelationshipKind::Observes,
            });
            fragment.text.push(FaceText {
                subject: identity.clone(),
                text: plot.title.clone(),
            });
            fragment.properties.extend([
                property(
                    &identity,
                    "patchbay-selected",
                    FacePropertyValue::Flag(self.selected_plot == Some(index)),
                ),
                property(
                    &identity,
                    "observed-plan-id",
                    FacePropertyValue::Identity(plot.plan_id.as_str().into()),
                ),
                property(
                    &identity,
                    "observed-plot-state",
                    FacePropertyValue::Text(
                        match plot.state {
                            PatchbayActivePlotState::Playing => "playing",
                            PatchbayActivePlotState::Lulled => "lulled",
                        }
                        .into(),
                    ),
                ),
            ]);
        }
        if let Some(index) = self.selected_plot {
            graph_face::append(
                &self.plots[index].graph,
                &body,
                &mut graph_face::Content {
                    subjects: &mut fragment.subjects,
                    relationships: &mut fragment.relationships,
                    properties: &mut fragment.properties,
                    texts: &mut fragment.text,
                    disclosures: &mut fragment.disclosures,
                },
            );
        }
        // Graph composition may name an inspected Plot, but the Body's outer
        // Face owns that resident subject and its containment; never redeclare it.
        let owned_plots: Vec<String> = self
            .plots
            .iter()
            .map(|plot| format!("plot/{}", plot.graph.checked_plot_id.as_str()))
            .collect();
        fragment
            .subjects
            .retain(|subject| !owned_plots.contains(&subject.identity));
        fragment.relationships.retain(|relation| {
            !(relation.source == body
                && relation.kind == FaceRelationshipKind::Contains
                && owned_plots.contains(&relation.target))
        });
        fragment
            .disclosures
            .retain(|disclosure| !owned_plots.contains(&disclosure.subject));
        if let Some(topology) = &self.mask_topology {
            super::resident_mask_face::append(&mut fragment, &root, topology);
        }
        fragment.validate_bounds()?;
        Ok(fragment)
    }
}

fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}
