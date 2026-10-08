//! Ordinary typed startup constructors and finite HostCall offers for IPA.
use crate::semantic::*;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::*;
use conduit_plot::{
    rust_binding::NativeRustBinding, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
pub const PHONETIC_KIND: &str = "speech/phonetic-from-ipa";
pub const PHONEMIC_KIND: &str = "speech/phonemic-from-ipa";
pub const IPA_OPERATION: &str = "conduit.host/ipa-admission@1";
pub const IPA_RESOURCE: &str = "speech/ipa-admission";
pub const MAXIMUM_IPA_INPUT_BYTES: u32 = 262_144;
pub const MAXIMUM_IPA_OUTPUT_BYTES: u32 = 262_144;
pub const PHONETIC_IMPLEMENTATION: &str = "std/phonetic-from-ipa@1";
pub const PHONEMIC_IMPLEMENTATION: &str = "std/phonemic-from-ipa@1";
fn types(phonemic: bool) -> (StructuredInfoType, StructuredInfoType) {
    if phonemic {
        (
            SpeechPhonemicIpaRequest::semantic_type().unwrap(),
            SpeechPhonemicIpaOutcome::semantic_type().unwrap(),
        )
    } else {
        (
            SpeechPhoneticIpaRequest::semantic_type().unwrap(),
            SpeechPhoneticIpaOutcome::semantic_type().unwrap(),
        )
    }
}
pub fn ipa_kind(phonemic: bool) -> Kind {
    let (request, outcome) = types(phonemic);
    let request_profile = request.profile().unwrap().value_kind().clone();
    let output_profile = outcome.profile().unwrap().value_kind().clone();
    let mut kind = Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "request".into(),
            value_type: request_profile.clone(),
            has_default: false,
        }],
        shorthand: None,
        kind_id: kind_id(if phonemic {
            PHONEMIC_KIND
        } else {
            PHONETIC_KIND
        }),
        kind_contract_revision: KindIdentity::from("conduit.speech/ipa-admission@1"),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("outcome"),
            value_kind: output_profile.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "request".into(),
            default_value: ConfigurationValue::Structured(placeholder(phonemic)),
            rule: KindConfigurationRule::Structured {
                profile: request_profile,
            },
        }],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Output(port_id("outcome")),
            contract: CheckedValueContract::new(output_profile, MAXIMUM_IPA_OUTPUT_BYTES, vec![])
                .unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_IPA_OUTPUT_BYTES,
        },
    };
    if phonemic {
        let inventory = placeholder_inventory();
        let ty = SpeechInventory::semantic_type().unwrap();
        let profile = ty.profile().unwrap().value_kind().clone();
        kind.startup_parameters.push(FrontStartupParameter {
            name: "inventory".into(),
            value_type: profile.clone(),
            has_default: false,
        });
        kind.configuration.push(KindConfigurationField {
            key: "inventory".into(),
            default_value: ConfigurationValue::Structured(
                StructuredConfigurationValue::new(profile.clone(), inventory.encode().unwrap())
                    .unwrap(),
            ),
            rule: KindConfigurationRule::Structured { profile },
        });
    }
    kind
}
pub fn ipa_offer(phonemic: bool) -> CapabilityOffer {
    let kind = ipa_kind(phonemic);
    capability_offer_from_parts! {
        startup_parameters: kind.startup_parameters.clone(), shorthand: None,
        capability_id: CapabilityId::from(if phonemic { PHONEMIC_IMPLEMENTATION } else { PHONETIC_IMPLEMENTATION }),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(), inputs: kind.inputs.clone(), outputs: kind.outputs.clone(), semantic_contract: kind.semantic_contract(),
        implementation: ImplementationOffer { execution_profile_id: ExecutionProfileId::from("conduit.speech/ipa-admission@1"), implementation_id: ImplementationId::from(if phonemic { PHONEMIC_IMPLEMENTATION } else { PHONETIC_IMPLEMENTATION }), artifact_id: ArtifactId::from("conduit-std/ipa-admission@1") },
        host_calls: vec![HostCallRequirement { contract_id: HostCallContractId::from(IPA_OPERATION), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: MAXIMUM_IPA_INPUT_BYTES, maximum_output_bytes: MAXIMUM_IPA_OUTPUT_BYTES }],
        resource_requirements: vec![resource_requirement(IPA_RESOURCE, 1)], authority_requirements: Vec::new(), limits: kind.limits,
    }
}
pub fn install_ipa_catalog(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    for (phonemic, request_name, outcome_name) in [
        (
            false,
            "SpeechPhoneticIpaRequest",
            "SpeechPhoneticIpaOutcome",
        ),
        (true, "SpeechPhonemicIpaRequest", "SpeechPhonemicIpaOutcome"),
    ] {
        let (request, outcome) = types(phonemic);
        startup.ensure_structured_type(request_name, request)?;
        startup.ensure_structured_type(outcome_name, outcome)?;
        let mut parameters = vec![StartupParameterSignature {
            name: "request".into(),
            value_type: request_name.into(),
            default: None,
        }];
        if phonemic {
            startup.ensure_structured_type(
                "SpeechInventory",
                SpeechInventory::semantic_type().unwrap(),
            )?;
            parameters.push(StartupParameterSignature {
                name: "inventory".into(),
                value_type: "SpeechInventory".into(),
                default: None,
            });
        }
        startup.insert(KindSignature {
            kind: if phonemic {
                PHONEMIC_KIND
            } else {
                PHONETIC_KIND
            }
            .into(),
            startup_parameters: parameters,
        })?;
        profiles
            .insert_kind(ipa_kind(phonemic))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn placeholder(phonemic: bool) -> StructuredConfigurationValue {
    // Checked finite descriptor placeholders are never callable defaults.
    let p = SpeechEvidenceProvenance::new(
        "descriptor placeholder".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap();
    let unit = SpeechIpaUnitDefinition::new(
        SpeechIpaUnitId::new("placeholder/schwa".into()).unwrap(),
        SpeechIpaUnitKind::Segment,
        p.clone(),
        SpeechIpaSpelling::new("ə".into()).unwrap(),
    )
    .unwrap();
    let units = conduit_plot::rust_binding::BoundedSequence::try_from_iter([unit]).unwrap();
    let id = SpeechIpaNotationProfileId::new("placeholder/notation@1".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("placeholder/revision@1".into()).unwrap();
    let (profile, bytes) = if !phonemic {
        let profile =
            SpeechPhoneticIpaProfile::new(Default::default(), id, p.clone(), revision, units)
                .unwrap();
        let request = SpeechPhoneticIpaRequest::new("ə".into(), profile, p).unwrap();
        (
            types(false).0.profile().unwrap().value_kind().clone(),
            request.encode().unwrap(),
        )
    } else {
        let language = conduit_language::LanguageId::new("placeholder/language".into()).unwrap();
        let variety = conduit_language::LanguageVariety::new(
            conduit_language::VarietyId::new("placeholder/variety".into()).unwrap(),
            language.clone(),
        )
        .unwrap();
        let inventory_id = SpeechInventoryId::new("placeholder/inventory".into()).unwrap();
        let profile = SpeechIpaNotationProfile::new(
            Default::default(),
            id,
            inventory_id.clone(),
            p.clone(),
            revision.clone(),
            units,
            variety.clone(),
        )
        .unwrap();
        let inventory = SpeechInventory::new(
            inventory_id,
            language,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let request = SpeechPhonemicIpaRequest::new(
            inventory.identity().clone(),
            "ə".into(),
            Default::default(),
            Default::default(),
            profile,
            p,
            revision,
            variety,
        )
        .unwrap();
        (
            types(true).0.profile().unwrap().value_kind().clone(),
            request.encode().unwrap(),
        )
    };
    StructuredConfigurationValue::new(profile, bytes).expect("checked descriptor placeholder")
}

fn placeholder_inventory() -> SpeechInventory {
    SpeechInventory::new(
        SpeechInventoryId::new("placeholder/inventory".into()).unwrap(),
        conduit_language::LanguageId::new("placeholder/language".into()).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap()
}
