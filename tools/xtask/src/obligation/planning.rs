use super::{ObligationBasis, ObligationRefusal, VALUE_BYTES};
use conduit_core::{
    kind_id, port_id, ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, FaceStartupParameter, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    KindContractRevision, OfferGeneration, PortDescriptor, PortDirection, PortTemporal,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

pub(super) const SOURCE_KIND: &str = "repository/proof-catalog-obligation";
pub(super) const EXECUTE_KIND: &str = "repository/execute-proof-catalog";
const VALUE_KIND: &str = "repository/validation-obligation@1";
const CONTRACT_REVISION: &str = "conduit.repository/proof-catalog-obligation@1";
const EXECUTION_PROFILE: &str = "conduit.repository/kernel-hosted@1";
const HOST_OPERATION: &str = "conduit.repository/execute-proof-catalog@1";
const QUEUE_SLOTS: usize = 2;
const FIELDS: &[&str] = &[
    "commit",
    "command",
    "tool",
    "profile",
    "artifact",
    "digest",
    "proof-class",
];

pub(super) fn checked_plan(
    basis: &ObligationBasis,
) -> Result<
    (
        conduit_form::ExpandedCanonicalForm,
        conduit_core::Plan,
        HostAdvertisement,
    ),
    ObligationRefusal,
> {
    let (startup, profiles) = catalogs().map_err(|_| ObligationRefusal::StepFailed)?;
    let source = format!(
        "form proof-catalog-obligation {{\n    obligation: {SOURCE_KIND}(\"{}\", \"{}\", \"{}\", \"{}\", \"{}\", \"{}\", \"{}\")\n    execute: {EXECUTE_KIND}\n    obligation > execute\n}}\n",
        basis.source_commit,
        basis.command,
        basis.tool,
        basis.profile,
        basis.artifact,
        basis.artifact_digest,
        basis.proof_class.as_str(),
    );
    let syntax = parse_syntax_document(&source);
    let checked =
        check_syntax_document(&syntax, &startup).map_err(|_| ObligationRefusal::StepFailed)?;
    let expanded = expand_canonical_form(&checked, "proof-catalog-obligation", &profiles)
        .map_err(|_| ObligationRefusal::StepFailed)?;
    let advertisement = advertisement();
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .map_err(|_| ObligationRefusal::StepFailed)?;
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: QUEUE_SLOTS as u16,
            connection_byte_capacity: VALUE_BYTES * QUEUE_SLOTS as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| ObligationRefusal::StepFailed)?;
    Ok((expanded, plan, advertisement))
}

fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: SOURCE_KIND.into(),
            startup_parameters: FIELDS
                .iter()
                .map(|name| StartupParameterSignature {
                    name: (*name).into(),
                    value_type: "Text".into(),
                    default: None,
                })
                .collect(),
        })
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: EXECUTE_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert(KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
            inputs: vec![],
            outputs: vec![port(PortDirection::Output)],
            configuration: FIELDS
                .iter()
                .map(|name| ConfigurationField {
                    key: (*name).into(),
                    default_value: conduit_core::ConfigurationValue::Text(String::new()),
                    validation: ConfigurationRule::TextBytes { maximum: 256 },
                })
                .collect(),
        })
        .map_err(|error| error.to_string())?;
    profiles
        .insert(KindDefinition {
            kind_id: kind_id(EXECUTE_KIND),
            kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
            inputs: vec![port(PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())?;
    Ok((startup, profiles))
}

fn port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("obligation"),
        value_kind: kind_id(VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("repository-validation-host"),
        boot_id: BootId::from("repository-validation-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("repository-validation"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![source_offer(), execute_offer()],
    }
}

fn source_offer() -> CapabilityOffer {
    CapabilityOffer {
        capability_id: CapabilityId::from("repository-proof-obligation"),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        implementation: implementation("repository/proof-obligation-source@1"),
        inputs: vec![],
        outputs: vec![port(PortDirection::Output)],
        startup_parameters: FIELDS
            .iter()
            .map(|name| FaceStartupParameter {
                name: (*name).into(),
                value_type: "Text".into(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: limits(),
    }
}

fn execute_offer() -> CapabilityOffer {
    CapabilityOffer {
        capability_id: CapabilityId::from("repository-execute-proof-catalog"),
        kind_id: kind_id(EXECUTE_KIND),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        implementation: implementation("repository/execute-proof-catalog@1"),
        inputs: vec![port(PortDirection::Input)],
        outputs: vec![],
        startup_parameters: vec![],
        shorthand: None,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(kind_id("repository/proof-catalog")),
            maximum_in_flight: 1,
            maximum_input_bytes: VALUE_BYTES,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: limits(),
    }
}

fn implementation(id: &str) -> conduit_core::ImplementationOffer {
    conduit_core::ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
        implementation_id: ImplementationId::from(id),
        artifact_id: ArtifactId::from(id),
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 2,
        max_queue_bytes: VALUE_BYTES * 2,
    }
}
