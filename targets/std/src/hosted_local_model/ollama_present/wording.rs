use super::*;
use conduit_presentation::{PresentationDisclosureLevel, PresentationRole};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WordingWire {
    proposal: GeneratedWordingProposal,
    #[serde(default)]
    suggested_action_identities: Vec<String>,
}

pub(super) fn finish_wording(
    prepared: PreparedPresent,
    provider_output: &str,
    identity: &LocalModelIdentity,
    sequence: u64,
    truncated: bool,
) -> Result<Vec<u8>, String> {
    if provider_output.is_empty() || provider_output.len() > MAX_RAW_PRESENTER_OUTPUT_BYTES {
        return Err("model wording output exceeded its retained byte bound".into());
    }
    let wire = serde_json::from_str::<WordingWire>(provider_output);
    let proposal = wire.as_ref().ok().map(|wire| wire.proposal.clone());
    let speech = if truncated {
        None
    } else {
        proposal.as_ref().and_then(|proposal| {
            crate::spoken_face_mask::select_spoken_outline(
                &prepared.request.semantic_data.presentation,
            )
            .ok()?
            .render_model_wording(&prepared.request.semantic_data.presentation, proposal)
            .ok()
        })
    };
    let mut correlations = Vec::new();
    let mut affordances = Vec::new();
    let mut valid = speech.is_some()
        && proposal.as_ref().is_some_and(|proposal| {
            useful_spoken_order(&prepared.request.semantic_data.presentation, proposal)
        });
    if let Ok(wire) = wire {
        if valid {
            for clause in &wire.proposal.clauses {
                let correlation = match clause {
                    GeneratedWordingClause::Text { index, subject, .. } => {
                        GeneratedSemanticCorrelation::Text {
                            index: *index,
                            subject: subject.clone(),
                        }
                    }
                    GeneratedWordingClause::Property {
                        index,
                        subject,
                        name,
                        ..
                    } => GeneratedSemanticCorrelation::Property {
                        index: *index,
                        subject: subject.clone(),
                        name: name.clone(),
                    },
                    GeneratedWordingClause::Action {
                        index, identity, ..
                    } => {
                        let action =
                            &prepared.request.semantic_data.presentation.actions[*index as usize];
                        affordances.push(
                            GeneratedActionAffordance::new(
                                identity.clone(),
                                prepared.request.semantic_data.source_presentation_revision,
                            )
                            .map_err(|error| format!("invalid spoken action: {error:?}"))?,
                        );
                        GeneratedSemanticCorrelation::Action {
                            index: *index,
                            identity: identity.clone(),
                            intent: action.intent.clone(),
                            target: action.target.clone(),
                        }
                    }
                };
                if !correlations.contains(&correlation) {
                    correlations.push(correlation);
                }
            }
            for identity in wire.suggested_action_identities {
                let Some((index, action)) = prepared
                    .request
                    .semantic_data
                    .presentation
                    .actions
                    .iter()
                    .enumerate()
                    .find(|(_, action)| {
                        action.identity == identity
                            && action.availability.is_available()
                            && crate::spoken_face_mask::select_spoken_outline(
                                &prepared.request.semantic_data.presentation,
                            )
                            .is_ok_and(|outline| outline.action_ids.contains(&identity))
                    })
                else {
                    valid = false;
                    break;
                };
                let correlation = GeneratedSemanticCorrelation::Action {
                    index: index as u32,
                    identity: identity.clone(),
                    intent: action.intent.clone(),
                    target: action.target.clone(),
                };
                if !correlations.contains(&correlation) {
                    correlations.push(correlation);
                }
                if !affordances
                    .iter()
                    .any(|affordance| affordance.action_identity() == &identity)
                {
                    affordances.push(
                        GeneratedActionAffordance::new(
                            identity,
                            prepared.request.semantic_data.source_presentation_revision,
                        )
                        .map_err(|error| format!("invalid suggested action: {error:?}"))?,
                    );
                }
            }
        }
    }
    if !valid {
        correlations.clear();
        affordances.clear();
    }
    let mut candidate = GeneratedManifestationCandidate {
        candidate_identity: String::new(),
        request_identity: prepared.request.request_identity.clone(),
        source_presentation_identity: prepared
            .request
            .semantic_data
            .source_presentation_identity
            .clone(),
        source_presentation_revision: prepared.request.semantic_data.source_presentation_revision,
        presenter_implementation_identity: conduit_ai::LOCAL_MODEL_IMPLEMENTATION.into(),
        provider_identity: format!("ollama/{}", identity.runtime_version),
        model_identity: format!(
            "{}/{}",
            identity.model_name, identity.model_content_identity
        ),
        template_contract_revision: prepared.request.policy.template_contract_revision.clone(),
        mask_contract_revision: conduit_presentation::SPOKEN_MASK_CONTRACT_REVISION.into(),
        generation_run_identity: format!("run/ollama-present/{sequence}"),
        disposition: if valid {
            GeneratedManifestationDisposition::Produced
        } else {
            GeneratedManifestationDisposition::Refused
        },
        content: speech.filter(|_| valid).map_or_else(Vec::new, |speech| {
            vec![GeneratedContentSegment {
                role: GeneratedContentRole::Speech,
                source_text_index: 0,
                bytes: speech.into_bytes(),
            }]
        }),
        affordances,
        correlations,
        raw_provider_output: Some(provider_output.into()),
        wording_proposal: proposal,
    };
    candidate.candidate_identity = candidate.digest();
    prepared
        .request
        .validate_candidate(&candidate)
        .map_err(|error| format!("invalid model wording candidate: {error:?}"))?;
    let encoded = serde_json::to_vec(&candidate).map_err(|error| error.to_string())?;
    if encoded.len() > prepared.request.bounds.maximum_output_bytes as usize {
        return Err("model wording candidate exceeded its admitted output bound".into());
    }
    Ok(encoded)
}

/// Exact grounding is necessary but a default utterance must also orient the
/// listener. Refuse a true Body-only sentence or a result placed before its
/// explicitly authored Context; the raw proposal remains inspectable.
fn useful_spoken_order(
    face: &conduit_presentation::Presentation,
    proposal: &GeneratedWordingProposal,
) -> bool {
    let disclosure = |subject: &str| {
        face.disclosures
            .iter()
            .find(|item| item.subject == subject)
            .map(|item| item.level)
    };
    let role = |subject: &str| {
        face.subjects
            .iter()
            .find(|item| item.identity == subject)
            .map(|item| &item.role)
    };
    let context =
        face.text.iter().enumerate().find(|(_, item)| {
            disclosure(&item.subject) == Some(PresentationDisclosureLevel::Context)
        });
    if let Some((index, _)) = context {
        if !matches!(proposal.clauses.first(), Some(GeneratedWordingClause::Text { index: selected, .. }) if *selected as usize == index)
        {
            return false;
        }
    }
    let has_application_result = face.text.iter().any(|item| {
        matches!(
            disclosure(&item.subject),
            None | Some(PresentationDisclosureLevel::Primary)
        ) && !matches!(role(&item.subject), Some(PresentationRole::Body))
    });
    !has_application_result
        || proposal.clauses.iter().any(|clause| {
            matches!(clause, GeneratedWordingClause::Text { index, .. }
                if face.text.get(*index as usize).is_some_and(|item|
                    matches!(disclosure(&item.subject), None | Some(PresentationDisclosureLevel::Primary))
                        && !matches!(role(&item.subject), Some(PresentationRole::Body))))
        })
}
