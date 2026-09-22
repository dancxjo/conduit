use crate::{
    install_speech_catalogs, speech_host_fixture, OutputCondition, ARTIFACT_WRITE_AUTHORITY,
    AUDIO_OUTPUT_AUTHORITY, AUDIO_PLAY_KIND,
};
use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseImplementationId,
    CapabilityId, HostCallContractId,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
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
    plan_speech_text(crate::SPECIMEN_TEXT, condition)
}

pub fn plan_speech_text(text: &str, condition: OutputCondition) -> Result<PlannedSpeech, String> {
    if text.is_empty() {
        return Err("speech Text must not be empty".into());
    }
    if text.len() > crate::MAXIMUM_TEXT_BYTES as usize {
        return Err("speech Text exceeds its finite byte bound".into());
    }
    let encoded = serde_json::to_string(text).map_err(|error| error.to_string())?;
    let source = format!(
        "form tongues_text_to_speech {{\n    tts: speech/synthesize\n    output: audio/play\n    {encoded} > tts > output\n}}\n"
    );
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_speech_catalogs(&mut startup, &mut profile)?;
    let literal = conduit_text::text_literal_semantics();
    startup.insert(conduit_form::KindSignature {
        kind: literal.kind_id.as_str().into(),
        startup_parameters: vec![conduit_form::StartupParameterSignature {
            name: "value".into(),
            value_type: "Text".into(),
            default: None,
        }],
    })?;
    profile
        .insert(conduit_form::KindProjection {
            kind_id: literal.kind_id,
            kind_contract_revision: conduit_core::KindIdentity::from(
                conduit_text::TEXT_LITERAL_CONTRACT_REVISION,
            ),
            inputs: literal.inputs,
            outputs: literal.outputs,
            configuration: literal
                .configuration
                .into_iter()
                .map(|field| conduit_form::KindConfigurationField {
                    key: field.key.into(),
                    default_value: field.default_value,
                    rule: conduit_form::KindConfigurationRule::TextBytes {
                        maximum: field.maximum_text_bytes,
                    },
                })
                .collect(),
        })
        .map_err(|error| error.to_string())?;

    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_form(&checked, "tongues_text_to_speech", &profile)
        .map_err(|error| error.to_string())?;
    let mut fixture = speech_host_fixture(condition);
    let literal_contract = conduit_text::text_literal_semantics();
    let mut literal_offer = conduit_core::CapabilityOffer {
        startup_parameters: vec![conduit_core::FrontStartupParameter {
            name: "value".into(),
            value_type: conduit_core::kind_id("value/text"),
            has_default: false,
        }],
        shorthand: None,
        capability_id: conduit_core::CapabilityId::from("tongues/text-literal-fixture@1"),
        kind_id: literal_contract.kind_id,
        kind_contract_revision: literal_contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: conduit_core::ExecutionProfileId::from(
                "tongues/speech-fixture@1",
            ),
            implementation_id: conduit_core::ImplementationId::from(
                "tongues/text-literal-fixture@1",
            ),
            artifact_id: conduit_core::ArtifactId::from("tongues/speech-fixture@1"),
        },
        inputs: literal_contract.inputs,
        outputs: literal_contract.outputs,
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: literal_contract.limits,
    };
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
        host_call_contract_id: HostCallContractId::from(requirement.host_call_contract_id.as_str()),
        subject_kind: kind_id(AUDIO_PLAY_KIND),
        host_id: fixture.advertisement.host_id.clone(),
        boot_id: fixture.advertisement.boot_id.clone(),
        capability_id: CapabilityId::from(output.capability_id.as_str()),
    };
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        std::slice::from_ref(&fixture.advertisement),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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

    #[test]
    fn bounded_text_is_part_of_exact_speech_plan_identity() {
        let first = plan_speech_text("Rosehip says hello.", OutputCondition::DegradedWavArtifact)
            .expect("bounded Text plans");
        let escaped = plan_speech_text(
            "Rosehip says \"hello\".\n",
            OutputCondition::DegradedWavArtifact,
        )
        .expect("escaped bounded Text plans");
        assert_ne!(first.plan.plan_id, escaped.plan.plan_id);
        assert!(plan_speech_text("", OutputCondition::DegradedWavArtifact).is_err());
        assert!(plan_speech_text(
            &"x".repeat(crate::MAXIMUM_TEXT_BYTES as usize + 1),
            OutputCondition::DegradedWavArtifact,
        )
        .is_err());
    }
}
