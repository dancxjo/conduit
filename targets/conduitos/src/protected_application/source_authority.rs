//! Check immutable inspection custody without duplicating the private actor.
use super::*;
use conduit_face::{FaceFragment, FacePropertyValue, FaceRelationshipKind};

pub(super) fn check_fragment(
    fragment: &FaceFragment,
    graph: &PatchbayGraph,
) -> Result<(), Refusal> {
    let subject = alloc::format!("plot/{}", graph.checked_plot_id.as_str());
    let exact = |name: &str, value: FacePropertyValue| {
        let mut entries = fragment
            .properties
            .iter()
            .filter(|property| property.subject == subject && property.name == name);
        entries
            .next()
            .is_some_and(|property| property.value == value)
            && entries.next().is_none()
    };
    let optional_identity = |name: &str, identity: &str| {
        !fragment
            .properties
            .iter()
            .any(|property| property.subject == subject && property.name == name)
            || exact(name, FacePropertyValue::Identity(identity.into()))
    };
    if !optional_identity("source-document-id", graph.source_document_id.as_str())
        || !optional_identity("expanded-plot-id", graph.expanded_plot_id.as_str())
        || !exact("inspected-plan-present", FacePropertyValue::Flag(false))
        || !exact("inspected-play-present", FacePropertyValue::Flag(false))
        || !exact(
            "inspected-source-document-id",
            FacePropertyValue::Identity(graph.source_document_id.as_str().into()),
        )
        || !exact(
            "inspected-expanded-plot-id",
            FacePropertyValue::Identity(graph.expanded_plot_id.as_str().into()),
        )
        || !exact(
            "observed-plot-state",
            FacePropertyValue::Text("checked-source".into()),
        )
        || fragment.properties.iter().any(|property| {
            property.subject == subject
                && matches!(
                    property.name.as_str(),
                    "observed-plan-id" | "observed-play-id"
                )
        })
        || fragment.relationships.iter().any(|relation| {
            relation.target == subject
                && relation.source.starts_with("body/")
                && relation.kind == FaceRelationshipKind::Contains
        })
    {
        return Err(Refusal::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{FaceContributionBasis, FaceProperty, FaceRelationship};

    #[test]
    fn root_checks_exact_source_custody_and_explicit_absence() {
        let expanded = conduitos::native_workset::checked(
            conduitos::native_workset::NativePlot::KeyboardCanvas,
        )
        .unwrap();
        let planned = PatchbayGraph::from_expanded(&expanded).unwrap();
        let source: PatchbayGraph = serde_json::from_str(include_str!(
            "../../application-core/tests/fixtures/vision-graph.json"
        ))
        .unwrap();
        let mut port = patchbay_application::PatchbayApplicationPort::open_graph(
            planned,
            "plan/resident".into(),
            "plan/body".into(),
        )
        .unwrap()
        .with_prepared_graph_capacity(2)
        .unwrap();
        port.append_prepared_source_graph(source.clone(), &"plan/body".into())
            .unwrap();
        port.select_prepared_plot(1).unwrap();
        let fragment = port
            .observe_fragment(
                FaceContributionBasis {
                    checked_plot_id: "checked/owner".into(),
                    plan_id: "plan/body".into(),
                    active_play_id: "play/owner".into(),
                    required_interaction_context: None,
                },
                &serde_json::from_str("\"body/source-custody\"").unwrap(),
            )
            .unwrap();
        check_fragment(&fragment, &source).unwrap();
        let subject = alloc::format!("plot/{}", source.checked_plot_id.as_str());
        for case in 0..6 {
            let mut forged = fragment.clone();
            match case {
                0 => {
                    forged
                        .properties
                        .iter_mut()
                        .find(|property| {
                            property.subject == subject
                                && property.name == "inspected-source-document-id"
                        })
                        .unwrap()
                        .value = FacePropertyValue::Identity("source/other".into())
                }
                1 => forged.properties.push(FaceProperty {
                    subject: subject.clone(),
                    name: "observed-plan-id".into(),
                    value: FacePropertyValue::Identity("plan/fabricated".into()),
                }),
                2 => forged.properties.push(FaceProperty {
                    subject: subject.clone(),
                    name: "inspected-play-present".into(),
                    value: FacePropertyValue::Flag(false),
                }),
                3 => forged.relationships.push(FaceRelationship {
                    source: "body/source-custody".into(),
                    target: subject.clone(),
                    kind: FaceRelationshipKind::Contains,
                }),
                4 => {
                    forged
                        .properties
                        .iter_mut()
                        .find(|property| {
                            property.subject == subject && property.name == "source-document-id"
                        })
                        .unwrap()
                        .value = FacePropertyValue::Identity("source/contradiction".into())
                }
                5 => forged.properties.push(FaceProperty {
                    subject: subject.clone(),
                    name: "expanded-plot-id".into(),
                    value: FacePropertyValue::Identity(source.expanded_plot_id.as_str().into()),
                }),
                _ => unreachable!(),
            }
            assert_eq!(check_fragment(&forged, &source), Err(Refusal::Malformed));
        }
    }
}
