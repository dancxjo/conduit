//! Exact semantic contracts and compiled, authority-free constructor offers.
use super::{IpaConstructor, PROFILE, REVISION};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub fn install(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) -> Result<(), String> {
    for constructor in IpaConstructor::ALL {
        startup.insert(KindSignature {
            kind: constructor.kind().into(),
            startup_parameters: constructor
                .parameters()
                .into_iter()
                .map(|(name, value_type, _)| StartupParameterSignature {
                    name: name.into(),
                    value_type: value_type.into(),
                    default: None,
                })
                .collect(),
        })?;
        profile
            .insert_kind(contract(constructor))
            .map_err(|e| format!("{e}"))?;
    }
    Ok(())
}

pub fn contract(constructor: IpaConstructor) -> Kind {
    let parameters = constructor.parameters();
    let output_kind = constructor
        .output_type()
        .profile()
        .expect("finite output")
        .value_kind()
        .clone();
    Kind {
        kind_id: kind_id(constructor.kind()),
        kind_contract_revision: KindIdentity::from(REVISION),
        startup_parameters: parameters
            .iter()
            .map(|(name, _, ty)| FrontStartupParameter {
                name: (*name).into(),
                value_type: ty.profile().expect("finite parameter").value_kind().clone(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: output_kind.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: parameters
            .into_iter()
            .map(|(name, _, ty)| KindConfigurationField {
                key: name.into(),
                // Required startup has no authored default. This finite checked
                // placeholder only supplies the configuration field's schema.
                default_value: super::placeholder::configuration(name),
                rule: KindConfigurationRule::Structured {
                    profile: ty.profile().expect("finite parameter").value_kind().clone(),
                },
            })
            .collect(),
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Output(port_id("value")),
            contract: CheckedValueContract::new(
                output_kind,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                vec![],
            )
            .expect("finite output"),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

pub fn offer(constructor: IpaConstructor) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract(constructor),
        Back {
            capability_id: CapabilityId::from(constructor.implementation()),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(constructor.implementation()),
            artifact_id: ArtifactId::from(format!(
                "{}/{}",
                constructor.implementation(),
                crate::semantic::IPA_CONSTRUCTOR_SOURCE_ID
            )),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
