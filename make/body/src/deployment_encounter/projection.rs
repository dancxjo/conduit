use super::*;
use conduit_core::{kind_id, ValueConstraint};
use conduit_face::*;

pub(super) fn project(
    e: &DeploymentEncounter,
    basis: &DeploymentEncounterBasis,
) -> Result<Face, DeploymentEncounterRefusal> {
    use DeploymentEncounterRefusal as E;
    let birth_basis = basis.birth_basis()?;
    let mut base = e.draft.birth().face(&birth_basis).map_err(E::BirthFace)?;
    let birth_availability = base
        .actions
        .iter()
        .find(|a| a.identity == "creche.birth")
        .map(|a| a.availability.clone())
        .ok_or(E::InvalidBasis)?;
    let root = format!("birth/{}", basis.encounter_id);
    let article = format!("{root}/article");
    let navigation = format!("{root}/navigation");
    if e.page == DeploymentPage::Body {
        base.actions.retain(|a| a.identity != "creche.birth");
        // The producer is the current Body's Crèche Plot, not a bare Host.
        if let Some(subject) = base.subjects.iter_mut().find(|s| s.identity == root) {
            subject.role = FaceRole::Plot;
        }
    } else {
        base.subjects = vec![
            FaceSubject {
                identity: root.clone(),
                role: FaceRole::Plot,
                name: "Crèche".into(),
            },
            FaceSubject {
                identity: article.clone(),
                role: FaceRole::Semantic(kind_id("document/article")),
                name: match e.page {
                    DeploymentPage::Hosts => "A first home",
                    _ => "Make its machinery",
                }
                .into(),
            },
            FaceSubject {
                identity: navigation.clone(),
                role: FaceRole::Semantic(kind_id("document/navigation")),
                name: "Preparation actions".into(),
            },
        ];
        base.relationships = vec![contains(&root, &article), contains(&root, &navigation)];
        base.actions.clear();
        base.properties.clear();
        base.text.clear();
        base.disclosures.clear();
        base.disclosures.push(FaceDisclosure {
            subject: root.clone(),
            level: FaceDisclosureLevel::Primary,
        });
        base.properties.extend([
            FaceProperty {
                subject: root.clone(),
                name: "host-id".into(),
                value: FacePropertyValue::Identity(basis.host_id.as_str().into()),
            },
            FaceProperty {
                subject: root.clone(),
                name: "boot-id".into(),
                value: FacePropertyValue::Identity(basis.boot_id.as_str().into()),
            },
        ]);
        base.text.push(FaceText {
            subject: article.clone(),
            text: format!("New Body: {}", e.draft.birth().friendly_name()),
        });
        if e.page == DeploymentPage::Hosts {
            let hosts = e
                .draft
                .construction()
                .hosts()
                .iter()
                .map(|h| h.description.name.as_str())
                .collect::<Vec<_>>();
            choice(
                &mut base,
                &article,
                "creche.deploy.first",
                "First Host",
                e.draft.first_host(),
                &hosts,
            )?;
            choice(
                &mut base,
                &article,
                "creche.deploy.scope",
                "Images to prepare",
                if e.draft.scope() == DeploymentScope::AllHosts {
                    "all"
                } else {
                    "first"
                },
                &["first", "all"],
            )?;
            let profile_ids = e
                .profiles
                .iter()
                .map(|p| p.configuration_id())
                .collect::<Vec<_>>();
            for (index, host) in e.draft.construction().hosts().iter().enumerate() {
                text_field(
                    &mut base,
                    &article,
                    &format!("creche.deploy.host-name.{index}"),
                    &format!("Host name: {}", host.description.name),
                    &host.description.name,
                )?;
                choice(
                    &mut base,
                    &article,
                    &format!("creche.deploy.profile.{index}"),
                    &format!("Target profile: {}", host.description.name),
                    host.configuration.configuration_id(),
                    &profile_ids,
                )?;

                let id = format!("{root}/host/{index}");
                base.subjects.push(FaceSubject {
                    identity: id.clone(),
                    role: FaceRole::Info,
                    name: host.description.name.clone(),
                });
                base.relationships.push(contains(&article, &id));
                base.text.push(FaceText {
                    subject: id,
                    text: format!(
                        "Target: {}. Output: {:?}. Profile: {}.",
                        host.configuration.profile().target.key(),
                        host.description.spore.output,
                        host.configuration.configuration_id()
                    ),
                });
            }
        } else {
            choice(
                &mut base,
                &article,
                "creche.deploy.source",
                "Image source",
                if e.draft.image_source() == DeploymentImageSource::SourceBuild {
                    "build"
                } else {
                    "release"
                },
                &["release", "build"],
            )?;
            base.text.push(FaceText { subject: article.clone(), text: "Reviewed release: obtain verified matching artifacts. Source build: request the selected profile from an admitted builder. Source, toolchain and carrier authority must be reviewed by the owning Host operation.".into() });
            base.text.push(FaceText { subject: article.clone(), text: format!("First Host: {}. Output scope: {:?}. No image, boot or joined Host has been observed by this review.", e.draft.first_host(), e.draft.scope()) });
        }
    }
    if e.page != DeploymentPage::Body {
        base.actions.push(action(
            "creche.deploy.back",
            &navigation,
            "Back",
            FaceDisclosureLevel::CurrentAction,
        ));
    }
    base.actions.push(if e.page == DeploymentPage::Make {
        action(
            "creche.deploy.review",
            &navigation,
            "Review image preparation",
            FaceDisclosureLevel::Primary,
        )
    } else {
        action(
            "creche.deploy.next",
            &navigation,
            if e.page == DeploymentPage::Body {
                "Choose its first Host"
            } else {
                "Review how to make it"
            },
            FaceDisclosureLevel::Primary,
        )
    });
    if e.page == DeploymentPage::Body {
        if let Some(next) = base
            .actions
            .iter_mut()
            .find(|a| a.identity == "creche.deploy.next")
        {
            next.availability = birth_availability;
        }
    }
    Face::new_with_semantics(
        e.revision,
        basis.execution.clone(),
        base.subjects,
        base.relationships,
        base.properties,
        base.text,
        base.actions,
        base.disclosures,
    )
    .map_err(E::Projection)
}
fn contains(parent: &str, child: &str) -> FaceRelationship {
    FaceRelationship {
        source: parent.into(),
        target: child.into(),
        kind: FaceRelationshipKind::Contains,
    }
}
fn action(id: &str, target: &str, label: &str, disclosure: FaceDisclosureLevel) -> FaceAction {
    FaceAction {
        identity: id.into(),
        intent: format!("conduit.intent/{id}@1"),
        target: target.into(),
        name: label.into(),
        arguments: vec![],
        disclosure,
        availability: FaceActionAvailability::Available,
    }
}
fn choice(
    face: &mut Face,
    parent: &str,
    id: &str,
    label: &str,
    current: &str,
    members: &[&str],
) -> Result<(), DeploymentEncounterRefusal> {
    let mut argument = FaceActionArgument::text("value".into(), label.into(), 1, 256)
        .map_err(|_| DeploymentEncounterRefusal::InvalidBasis)?;
    let mut encoded = members
        .iter()
        .map(|v| v.as_bytes().to_vec())
        .collect::<Vec<_>>();
    encoded.sort();
    encoded.dedup();
    argument
        .contract
        .constraints
        .push(ValueConstraint::CanonicalMembership {
            members: encoded,
            negated: false,
        });
    field(
        face,
        parent,
        id,
        label,
        current,
        argument,
        format!(
            "Current choice: {current}. Choices: {}.",
            members.join(", ")
        ),
    )
}
fn text_field(
    face: &mut Face,
    parent: &str,
    id: &str,
    label: &str,
    current: &str,
) -> Result<(), DeploymentEncounterRefusal> {
    let argument = FaceActionArgument::text("value".into(), label.into(), 1, 64)
        .map_err(|_| DeploymentEncounterRefusal::InvalidBasis)?;
    field(
        face,
        parent,
        id,
        label,
        current,
        argument,
        format!("Current name: {current}"),
    )
}
fn field(
    face: &mut Face,
    parent: &str,
    id: &str,
    label: &str,
    current: &str,
    argument: FaceActionArgument,
    summary: String,
) -> Result<(), DeploymentEncounterRefusal> {
    face.subjects.push(FaceSubject {
        identity: id.into(),
        role: FaceRole::TextEntry,
        name: label.into(),
    });
    face.relationships.push(contains(parent, id));
    face.properties.extend([
        FaceProperty {
            subject: id.into(),
            name: "current-value".into(),
            value: FacePropertyValue::TypedValue {
                contract: argument.contract.clone(),
                bytes: current.as_bytes().to_vec(),
            },
        },
        FaceProperty {
            subject: id.into(),
            name: "value-change-action".into(),
            value: FacePropertyValue::Identity(id.into()),
        },
    ]);
    face.text.push(FaceText {
        subject: id.into(),
        text: summary,
    });
    let mut a = action(id, id, label, FaceDisclosureLevel::CurrentAction);
    a.arguments.push(argument);
    face.actions.push(a);
    Ok(())
}
