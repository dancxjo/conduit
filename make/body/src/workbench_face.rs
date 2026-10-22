//! Exact owner-composed workbench Face shared by coordinator and runtime.
//! This is a pure projection of validated Body evidence, not input authority.
use crate::BodyEvidenceAttachment;
use conduit_face::{
    Face, FaceAction, FaceActionAvailability, FaceBasis, FaceDisclosure, FaceDisclosureLevel,
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};
pub fn project_body_workbench_face(
    revision: u64,
    attachment: &BodyEvidenceAttachment,
    reviewed_plots: &[crate::ReviewedBodyPlot],
) -> Result<Face, String> {
    let evidence = attachment.evidence();
    let body_identity = format!("body/{}", evidence.body_id.as_str());
    let workset = evidence
        .body
        .effective_workset()
        .map_err(|error| format!("{error:?}"))?;
    let mut subjects = vec![FaceSubject {
        identity: body_identity.clone(),
        role: FaceRole::Body,
        name: evidence.friendly_name.clone(),
    }];
    let mut relationships = Vec::new();
    let mut properties = vec![
        identity_property(&body_identity, "body-id", evidence.body_id.as_str()),
        FaceProperty {
            subject: body_identity.clone(),
            name: "workload-revision".into(),
            value: FacePropertyValue::Count(evidence.body.workload_revision),
        },
    ];
    let mut plot_identities = Vec::with_capacity(workset.len());
    let lifecycle_action = match evidence.body.state {
        conduit_body::BodyState::Lulled => Some(("wake", "Wake", "conduit.intent/wake@1")),
        conduit_body::BodyState::Awake { .. } => Some(("lull", "Lull", "conduit.intent/lull@1")),
        conduit_body::BodyState::Fulfilled { .. } => None,
    };
    let mut actions = lifecycle_action
        .into_iter()
        .map(|(action_name, action_label, action_intent)| FaceAction {
            identity: format!("action/{action_name}/{body_identity}"),
            intent: action_intent.into(),
            target: body_identity.clone(),
            name: action_label.into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        })
        .collect::<Vec<_>>();
    for plot in workset.plots() {
        let plot_identity = format!("plot/{}", plot.checked_plot_id.as_str());
        let label = plot.checked_plot_id.as_str().to_owned();
        subjects.push(FaceSubject {
            identity: plot_identity.clone(),
            role: FaceRole::Plot,
            name: label.clone(),
        });
        relationships.push(FaceRelationship {
            source: body_identity.clone(),
            target: plot_identity.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        properties.push(identity_property(
            &plot_identity,
            "source-document-id",
            plot.source_document_id.as_str(),
        ));
        properties.push(identity_property(
            &plot_identity,
            "checked-plot-id",
            plot.checked_plot_id.as_str(),
        ));
        actions.push(FaceAction {
            identity: format!(
                "action/remove-plot/{}/{}",
                plot.checked_plot_id.as_str(),
                evidence.body.workload_revision
            ),
            intent: "conduit.intent/remove-plot@1".into(),
            target: plot_identity.clone(),
            name: "Remove from Body".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: match &evidence.body.state {
                conduit_body::BodyState::Lulled => FaceActionAvailability::Available,
                conduit_body::BodyState::Awake { .. } => {
                    FaceActionAvailability::Unavailable {
                        reason_code: "body-awake".into(),
                        explanation: "Lull the body before changing its active plot workload."
                            .into(),
                    }
                }
                conduit_body::BodyState::Fulfilled { .. } => {
                    FaceActionAvailability::Unavailable {
                        reason_code: "body-fulfilled".into(),
                        explanation:
                            "A fulfilled Body retains its biography but cannot change its plot workload."
                                .into(),
                    }
                }
            },
        });
        plot_identities.push(plot_identity);
    }
    let reviewed = crate::project_reviewed_body_plots(
        reviewed_plots,
        &workset,
        &evidence.body.state,
        evidence.body.workload_revision,
    )?;
    subjects.extend(reviewed.subjects);
    properties.extend(reviewed.properties);
    actions.extend(reviewed.actions);
    plot_identities.extend(reviewed.disclosures.iter().map(|item| item.subject.clone()));
    let mut text = vec![FaceText {
        subject: body_identity.clone(),
        text: format!(
            "{} is a durable body running {} current plot(s) at workload revision {}.",
            evidence.friendly_name,
            workset.len(),
            evidence.body.workload_revision,
        ),
    }];
    for part in &evidence.membership.parts {
        let part_identity = format!("part/{}", part.part_id.as_str());
        subjects.push(FaceSubject {
            identity: part_identity.clone(),
            role: FaceRole::Part,
            name: format!("Body Part {}", part.part_id.as_str()),
        });
        relationships.push(FaceRelationship {
            source: body_identity.clone(),
            target: part_identity.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        if let Some(current) = &part.current {
            let host_identity = format!("host/{}", current.host_id.as_str());
            subjects.push(FaceSubject {
                identity: host_identity.clone(),
                role: FaceRole::Host,
                name: format!("Current host {}", current.host_id.as_str()),
            });
            relationships.push(FaceRelationship {
                source: part_identity.clone(),
                target: host_identity.clone(),
                kind: FaceRelationshipKind::Realizes,
            });
            properties.push(identity_property(
                &host_identity,
                "boot-id",
                current.boot_id.as_str(),
            ));
        }
    }
    for record in &evidence.records {
        let sign_identity = format!("sign/{}", record.sign_id.as_str());
        subjects.push(FaceSubject {
            identity: sign_identity.clone(),
            role: FaceRole::Sign,
            name: format!("Body biography evidence sequence {}", record.sequence),
        });
        relationships.push(FaceRelationship {
            source: body_identity.clone(),
            target: sign_identity.clone(),
            kind: FaceRelationshipKind::Observes,
        });
        properties.push(FaceProperty {
            subject: sign_identity.clone(),
            name: "evidence-sequence".into(),
            value: FacePropertyValue::Count(record.sequence),
        });
        text.push(FaceText {
            subject: sign_identity,
            text: format!("{:?}", record.kind),
        });
    }
    Face::new_with_semantics(
        revision,
        FaceBasis {
            body_id: Some(evidence.body_id.clone()),
            wake_id: match &evidence.body.state {
                conduit_body::BodyState::Awake { wake_id } => Some(wake_id.clone()),
                conduit_body::BodyState::Lulled => None,
                conduit_body::BodyState::Fulfilled { .. } => None,
            },
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: evidence
                .records
                .iter()
                .map(|record| record.sign_id.clone())
                .collect(),
        },
        subjects,
        relationships,
        properties,
        text,
        actions,
        vec![FaceDisclosure {
            subject: body_identity,
            level: FaceDisclosureLevel::Primary,
        }]
        .into_iter()
        .chain(plot_identities.into_iter().map(|subject| FaceDisclosure {
            subject,
            level: FaceDisclosureLevel::Context,
        }))
        .collect(),
    )
    .map_err(|error| format!("{error:?}"))
}

fn identity_property(subject: &str, name: &str, identity: &str) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value: FacePropertyValue::Identity(identity.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, BodyWorkset, ResidentPlot};
    use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};

    fn attachment(source: &str) -> BodyEvidenceAttachment {
        let body = Body::born_with_plots(
            BodyWorkset::one(ResidentPlot::new(
                SourceDocumentId::from(source),
                CheckedPlotId::from("checked/note"),
            ))
            .unwrap(),
            1,
            SignId::from("sign/born"),
        )
        .unwrap();
        let membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let evidence = BodyBiographyEvidence::born(body, membership, "Talvi".into()).unwrap();
        BodyEvidenceAttachment::open_serialized(
            &serde_json::to_vec(&evidence).unwrap(),
            crate::BodyEvidenceEntrance::ExternalReader,
        )
        .unwrap()
    }
    #[test]
    fn canonical_projection_retains_body_source_inventory_and_revision_meaning() {
        let original = attachment("source/note");
        let reviewed = vec![crate::ReviewedBodyPlot {
            label: "Note".into(),
            source_document_id: "source/note".into(),
            checked_plot_id: "checked/note".into(),
        }];
        let face = project_body_workbench_face(7, &original, &reviewed).unwrap();
        face.validate().unwrap();
        assert_eq!(face.revision, 7);
        assert_eq!(
            face.basis.body_id.as_ref(),
            Some(&original.evidence().body_id)
        );
        assert!(face.basis.source_document_id.is_none());
        assert!(face
            .subjects
            .iter()
            .any(|subject| subject.role == FaceRole::Body && subject.name == "Talvi"));
        assert!(face
            .properties
            .iter()
            .any(|property| property.name == "source-document-id"
                && property.value == FacePropertyValue::Identity("source/note".into())));
        assert_ne!(
            face,
            project_body_workbench_face(8, &original, &reviewed).unwrap()
        );
        let changed_source = attachment("source/changed");
        assert!(project_body_workbench_face(7, &changed_source, &reviewed).is_err());
        let mut changed_inventory = reviewed.clone();
        changed_inventory[0].source_document_id = "source/changed".into();
        assert_ne!(
            face,
            project_body_workbench_face(7, &changed_source, &changed_inventory).unwrap()
        );
        let mut duplicate = reviewed.clone();
        duplicate.extend(reviewed);
        assert!(project_body_workbench_face(7, &original, &duplicate).is_err());
    }
}
