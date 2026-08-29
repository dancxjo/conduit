use conduit_core::{
    kind_id, port_id, resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId,
    CapabilityOffer, ExecutionProfileId, FaceStartupParameter, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    ImplementationOffer, OfferGeneration, PlannerCapabilityOffer, PlannerLimits, PlannerProfileId,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_planner::BROWSER_PLANNER_PROFILE;

pub(super) const BOOK_LOCAL_BASE: &str = "conduit.base/local@1";
pub(super) const MORSE_HOST_OPERATION: &str = "conduit.host/text-to-morse@1";
pub(super) const INDICATOR_HOST_OPERATION: &str = "conduit.host/present-indicator@1";

pub(super) fn catalog(
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profile)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)?;
    Ok((startup, profile))
}

pub(super) fn advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/executable-book@1"),
        resources: vec![resource_offer(
            "browser/book-indicator",
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            limits: PlannerLimits {
                maximum_host_advertisements: 1,
                maximum_gears: 3,
                maximum_connections: 2,
                maximum_authority_grants: 0,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 0,
            },
        }],
        capabilities: vec![literal_offer(), morse_offer(), indicator_offer()],
    }
}

fn literal_offer() -> CapabilityOffer {
    let contract = conduit_text::text_literal_semantics();
    let mut offer = CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "value".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("browser/book-text-literal@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/book-text-literal@1"),
            implementation_id: ImplementationId::from("browser/book-text-literal@1"),
            artifact_id: ArtifactId::from("conduit-browser-runtime/executable-book@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    };
    // This local Plan uses one exact queue budget for both Cords. The literal
    // value remains bounded by its semantic contract, while its admitted queue
    // can carry the larger downstream Morse pattern budget.
    offer.limits.max_queue_bytes = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32;
    offer
}

fn morse_offer() -> CapabilityOffer {
    let contract = conduit_text::text_morse_semantics();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: conduit_text::MORSE_UNIT_MILLIS_KEY.into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: Some((port_id("text"), port_id("pattern"))),
        capability_id: CapabilityId::from("browser/book-text-morse@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/book-text-morse@1"),
            implementation_id: ImplementationId::from("browser/book-text-morse@1"),
            artifact_id: ArtifactId::from("conduit-browser-runtime/executable-book@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MORSE_HOST_OPERATION),
            target_kind: Some(kind_id("text/morse-pattern")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
            maximum_output_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn indicator_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::indicator_presentation_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("browser/book-indicator@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_core::KindContractRevision::from(
            conduit_semantic_catalog::INDICATOR_PRESENTATION_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/book-indicator@1"),
            implementation_id: ImplementationId::from("browser/dom-indicator@1"),
            artifact_id: ArtifactId::from("conduit-browser-runtime/executable-book@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(INDICATOR_HOST_OPERATION),
            target_kind: Some(kind_id("presentation/browser-indicator")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}
