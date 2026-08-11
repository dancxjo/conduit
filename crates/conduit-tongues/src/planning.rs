use crate::{
    install_speech_catalogs, speech_host_fixture, OutputCondition, ARTIFACT_WRITE_AUTHORITY,
    AUDIO_OUTPUT_AUTHORITY, AUDIO_PLAY_KIND,
};
use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, CapabilityId, ConnectionBase,
    HostOperationContractId,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
use conduit_runtime::lowering::{lower_plan_fragment, LoweredPlanFragment};
use std::collections::BTreeMap;

pub const SPEECH_FORM: &str = r#"form tongues_text_to_speech {
    tts: speech/synthesize
    output: audio/play
    "Hello from Tongues." > tts > output
}
"#;

pub struct PlannedSpeech {
    pub plan: conduit_core::Plan,
    pub lowered: LoweredPlanFragment,
}

pub fn plan_speech(condition: OutputCondition) -> Result<PlannedSpeech, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_speech_catalogs(&mut startup, &mut profile)?;
    let literal = conduit_std_catalog::text_literal_contract();
    startup.insert(conduit_form::KindSignature {
        kind: literal.kind_id.as_str().into(),
        startup_parameters: vec![conduit_form::StartupParameterSignature {
            name: "value".into(),
            value_type: "Text".into(),
            default: None,
        }],
    })?;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: literal.kind_id,
            kind_contract_revision: conduit_core::KindContractRevision::from(
                conduit_std_catalog::TEXT_LITERAL_CONTRACT_REVISION,
            ),
            inputs: literal.inputs,
            outputs: literal.outputs,
            configuration: literal
                .configuration
                .into_iter()
                .map(|field| conduit_form::ConfigurationField {
                    key: field.key,
                    default_value: field.default_value,
                    validation: match field.rule {
                        conduit_std_catalog::StandardConfigurationRule::TextBytes { maximum } => {
                            conduit_form::ConfigurationRule::TextBytes { maximum }
                        }
                        _ => unreachable!("text literal has one text rule"),
                    },
                })
                .collect(),
        })
        .map_err(|error| error.to_string())?;

    let syntax = parse_syntax_document(SPEECH_FORM);
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_form(&checked, "tongues_text_to_speech", &profile)
        .map_err(|error| error.to_string())?;
    let mut fixture = speech_host_fixture(condition);
    let mut literal_offer = conduit_std_catalog::text_literal_offer();
    literal_offer.limits.max_queue_bytes = crate::MAXIMUM_PCM_BYTES;
    fixture.advertisement.capabilities.push(literal_offer);
    let placements =
        default_expanded_placements(&expanded, std::slice::from_ref(&fixture.advertisement))
            .map_err(|error| error.to_string())?;
    let output = &fixture.advertisement.capabilities[1];
    let requirement = &output.authority_requirements[0];
    let authority = AuthorityGrant {
        grant_id: AuthorityGrantId::from("tongues/output-authority"),
        contract_id: AuthorityContractId::from(match condition {
            OutputCondition::PrimaryPlayback => AUDIO_OUTPUT_AUTHORITY,
            OutputCondition::DegradedWavArtifact => ARTIFACT_WRITE_AUTHORITY,
        }),
        host_operation_contract_id: HostOperationContractId::from(
            requirement.host_operation_contract_id.as_str(),
        ),
        subject_kind: kind_id(AUDIO_PLAY_KIND),
        host_id: fixture.advertisement.host_id.clone(),
        boot_id: fixture.advertisement.boot_id.clone(),
        capability_id: CapabilityId::from(output.capability_id.as_str()),
    };
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        std::slice::from_ref(&fixture.advertisement),
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: crate::MAXIMUM_PCM_BYTES,
            authority_grants: &[authority],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| error.to_string())?;
    let lowered = lower_plan_fragment(&plan.fragments[0]).map_err(|error| format!("{error:?}"))?;
    Ok(PlannedSpeech { plan, lowered })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_planner_seals_both_truthful_realizations() {
        for condition in [
            OutputCondition::PrimaryPlayback,
            OutputCondition::DegradedWavArtifact,
        ] {
            let planned = plan_speech(condition).expect("speech form plans and lowers");
            assert_eq!(planned.plan.fragments[0].placements.len(), 3);
            assert_eq!(planned.lowered.nodes.len(), 3);
            assert_eq!(planned.lowered.cords.len(), 2);
            let fixture = speech_host_fixture(condition);
            let fragment = &planned.plan.fragments[0];
            assert!(fragment.placements.iter().all(|placement| placement.host_id
                == fixture.advertisement.host_id
                && placement.boot_id == fixture.advertisement.boot_id));
            let output = fragment
                .placements
                .iter()
                .find(|placement| placement.kind_id.as_str() == AUDIO_PLAY_KIND)
                .unwrap();
            assert!(output
                .resources
                .iter()
                .any(|binding| binding.pool_id.as_str() == fixture.facts.output_base_pool_id));
            assert_eq!(fragment.connections.len(), 2);
            assert!(fragment
                .connections
                .iter()
                .all(|cord| cord.item_capacity == 1
                    && cord.byte_capacity == crate::MAXIMUM_PCM_BYTES));
            assert_eq!(output.authority.len(), 1);
        }
    }
}
