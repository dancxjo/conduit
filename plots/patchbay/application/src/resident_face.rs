//! Initial source-owned observation without an event or a fabricated interaction.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::BodyId;
use conduit_face::{
    FaceContributionBasis, FaceDisclosure, FaceDisclosureLevel, FaceFragment, FaceFragmentError,
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};

use super::{graph_face, PatchbayActivePlotState, PatchbayApplicationPort};

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
        self.observe_fragment_with_graph(basis, body, true)
    }
    pub(super) fn observe_fragment_with_graph(
        &self,
        basis: FaceContributionBasis,
        body: &BodyId,
        include_graph: bool,
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
                text: self.inventory_title().into(),
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
        if self.edit_requested {
            fragment.text.push(FaceText {
                subject: root.clone(),
                text:
                    "Edit requested against the exact inspected subject; no authority was inferred."
                        .into(),
            });
        }
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
            if plot.plan_id.is_none() {
                fragment.subjects.push(FaceSubject {
                    identity: identity.clone(),
                    role: FaceRole::Plot,
                    name: plot.title.clone(),
                });
            }
            fragment.text.push(FaceText {
                subject: identity.clone(),
                text: plot.title.clone(),
            });
            if let Some(plan) = &plot.plan_id {
                fragment.properties.push(property(
                    &identity,
                    "observed-plan-id",
                    FacePropertyValue::Identity(plan.as_str().into()),
                ));
            } else {
                fragment.properties.extend([
                    property(
                        &identity,
                        "inspected-plan-present",
                        FacePropertyValue::Flag(false),
                    ),
                    property(
                        &identity,
                        "inspected-play-present",
                        FacePropertyValue::Flag(false),
                    ),
                    property(
                        &identity,
                        "inspected-source-document-id",
                        FacePropertyValue::Identity(plot.graph.source_document_id.as_str().into()),
                    ),
                    property(
                        &identity,
                        "inspected-expanded-plot-id",
                        FacePropertyValue::Identity(plot.graph.expanded_plot_id.as_str().into()),
                    ),
                ]);
            }
            fragment.properties.extend([
                property(
                    &identity,
                    "patchbay-selected",
                    FacePropertyValue::Flag(self.selected_plot == Some(index)),
                ),
                property(
                    &identity,
                    "observed-plot-state",
                    FacePropertyValue::Text(
                        match plot.state {
                            PatchbayActivePlotState::Playing => "playing",
                            PatchbayActivePlotState::Lulled => "lulled",
                            PatchbayActivePlotState::CheckedSource => "checked-source",
                        }
                        .into(),
                    ),
                ),
            ]);
        }
        if let Some(index) = self.selected_plot.filter(|_| include_graph) {
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
        let inspected_plots: Vec<String> = self
            .plots
            .iter()
            .map(|plot| format!("plot/{}", plot.graph.checked_plot_id.as_str()))
            .collect();
        fragment.subjects.retain(|subject| {
            !self
                .plots
                .iter()
                .zip(&inspected_plots)
                .any(|(plot, identity)| plot.plan_id.is_some() && identity == &subject.identity)
        });
        fragment.relationships.retain(|relation| {
            !(relation.source == body
                && relation.kind == FaceRelationshipKind::Contains
                && inspected_plots.contains(&relation.target))
        });
        fragment.disclosures.retain(|disclosure| {
            !self
                .plots
                .iter()
                .zip(&inspected_plots)
                .any(|(plot, identity)| plot.plan_id.is_some() && identity == &disclosure.subject)
        });
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
