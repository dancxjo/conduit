//! Exact two-Host plan for delivering one Patchbay Presentation to a renderer.

use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, GearId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision, LineOffer, LineScope, LineSecurity, LinkLimits,
    OfferGeneration, Plan, PortDirection, PROTOCOL_VERSION,
};
use conduit_form::{parse, KindDefinition, ProfileCatalog};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_presentation::{
    renderer_inputs, renderer_kind_definition, MAX_RENDERER_VALUE_BYTES, RENDERER_CONTRACT_REVISION,
};
use std::collections::BTreeMap;

use crate::{renderer_execution::renderer_host, RendererAdapterIdentity, RendererAdapterKind};

pub const PRESENTATION_PROJECT_KIND: &str = "presentation/patchbay-project";
pub const PRESENTATION_PROJECT_CAPABILITY: &str = "patchbay-project";
pub const CROSS_HOST_SOURCE_GEAR: &str = "project";
pub const CROSS_HOST_RENDERER_GEAR: &str = "renderer";
pub const CROSS_HOST_MAXIMUM_FRAME_BYTES: u32 = MAX_RENDERER_VALUE_BYTES + 8_192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossHostRendererPlan {
    pub source_advertisement: HostAdvertisement,
    pub renderer_advertisement: HostAdvertisement,
    pub line: LineOffer,
    pub plan: Plan,
}

pub fn cross_host_renderer_plan(
    source_host_id: HostId,
    source_boot_id: BootId,
    renderer_identity: RendererAdapterIdentity,
) -> Result<CrossHostRendererPlan, String> {
    let source_advertisement = source_host(source_host_id.clone(), source_boot_id.clone());
    let renderer_advertisement = renderer_host(RendererAdapterKind::HtmlDomSvg, &renderer_identity);
    let form = renderer_form()?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from(format!("cross-host-patchbay/{CROSS_HOST_SOURCE_GEAR}")),
                PlacementChoice {
                    host_id: source_host_id.clone(),
                    capability_id: CapabilityId::from(PRESENTATION_PROJECT_CAPABILITY),
                },
            ),
            (
                GearId::from(format!("cross-host-patchbay/{CROSS_HOST_RENDERER_GEAR}")),
                PlacementChoice {
                    host_id: renderer_identity.host_id.clone(),
                    capability_id: CapabilityId::from("renderer-dom-svg"),
                },
            ),
        ]),
    };
    let line = websocket_line(
        source_host_id,
        source_boot_id,
        renderer_identity.host_id,
        renderer_identity.boot_id,
    );
    let plan = plan_with_line_offers(
        &form,
        &[source_advertisement.clone(), renderer_advertisement.clone()],
        &placements,
        &[BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        )],
        1,
        MAX_RENDERER_VALUE_BYTES,
        core::slice::from_ref(&line),
    )
    .map_err(|error| error.to_string())?;
    Ok(CrossHostRendererPlan {
        source_advertisement,
        renderer_advertisement,
        line,
        plan,
    })
}

fn renderer_form() -> Result<conduit_form::CheckedForm, String> {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(project_kind_definition())
        .map_err(|error| error.to_string())?;
    catalog
        .insert(renderer_kind_definition())
        .map_err(|error| error.to_string())?;
    parse(
        "form cross-host-patchbay {\n    project: presentation/patchbay-project\n    renderer: presentation/renderer\n    project.presentation > renderer.presentation\n}\n",
        &catalog,
    )
    .map_err(|error| error.to_string())
}

fn project_kind_definition() -> KindDefinition {
    let mut output = renderer_inputs().remove(0);
    output.direction = PortDirection::Output;
    KindDefinition {
        kind_id: PRESENTATION_PROJECT_KIND.into(),
        kind_contract_revision: KindContractRevision::from(RENDERER_CONTRACT_REVISION),
        inputs: Vec::new(),
        outputs: vec![output],
        configuration: Vec::new(),
    }
}

fn source_host(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    let mut output = renderer_inputs().remove(0);
    output.direction = PortDirection::Output;
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("presentation/source-host@1"),
        resources: Vec::new(),
        capabilities: vec![CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from(PRESENTATION_PROJECT_CAPABILITY),
            kind_id: PRESENTATION_PROJECT_KIND.into(),
            kind_contract_revision: KindContractRevision::from(RENDERER_CONTRACT_REVISION),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("presentation/project-hosted@1"),
                implementation_id: ImplementationId::from("patchbay/project-presentation@1"),
                artifact_id: ArtifactId::from("patchbay-model/project-presentation@1"),
            },
            inputs: Vec::new(),
            outputs: vec![output],
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
            },
        }],
        planner_capabilities: Vec::new(),
    }
}

fn websocket_line(
    source_host_id: HostId,
    source_boot_id: BootId,
    sink_host_id: HostId,
    sink_boot_id: BootId,
) -> LineOffer {
    let source = source_host(source_host_id, source_boot_id);
    let sink = HostAdvertisement {
        host_id: sink_host_id,
        boot_id: sink_boot_id,
        ..source.clone()
    };
    let mut line = conduit_core::process_owned_line_offer_with_limits(
        "patchbay-renderer/line/websocket",
        "patchbay-renderer/binding/websocket",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "patchbay-renderer/websocket-instance",
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAX_RENDERER_VALUE_BYTES,
            maximum_buffered_bytes: MAX_RENDERER_VALUE_BYTES,
            maximum_frame_bytes: CROSS_HOST_MAXIMUM_FRAME_BYTES,
        },
    );
    line.contract.scope = LineScope::LocalNetwork;
    line.contract.security = LineSecurity::PlaintextNetwork;
    line
}
