use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, Plan, PortDescriptor,
    PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};
use conduit_presentation::{GRAPHICS_SCENE_KIND, LAYOUT_FRAME_KIND, MAX_GRAPHICS_SCENE_BYTES};
use conduit_runtime::lowering::{LoweredPlanFragment, lower_plan_fragment};

use super::{DISPLAY_HOST_OPERATION, DISPLAY_KIND, LAYOUT_SINK_KIND, TEXT_SOURCE_KIND};

pub const FORM_SOURCE: &str = r#"form 0

conduitos-presentation-nucleus {
 source: conduitos.fixture/text-source
 show: presentation/text
 viewport: layout/viewport
 row: layout/row
 column: layout/column
 stack: layout/stack
 align: layout/align
 layout_sink: conduitos.fixture/layout-observe
 icon: presentation/icon
 frame: presentation/frame
 badge: presentation/badge
 rect: graphics/rect
 text: graphics/text
 glyph: graphics/icon
 display: conduitos.fixture/framebuffer-present
 source.text -> show.text
 viewport.width = 320
 viewport.height = 200
 viewport.children = 3
 viewport.child-width = 40
 viewport.child-height = 30
 row.gap = 4
 column.gap = 3
 align.horizontal = "center"
 align.vertical = "end"
 viewport.placements -> row.frame
 row.placements -> column.frame
 column.placements -> stack.frame
 stack.placements -> align.frame
 align.placements -> layout_sink.input
 icon.icon = "type"
 icon.accessibility-name = "Patchbay"
 frame.role = "panel"
 frame.accessibility-name = "Gear Face"
 badge.state = "ready"
 badge.accessibility-name = "ready"
 rect.style = "stroke"
 text.text = "r"
 glyph.icon = "type"
 icon.presented -> frame.content
 frame.presented -> badge.content
 badge.presented -> rect.input
 rect.scene -> text.input
 text.scene -> glyph.input
 glyph.scene -> display.input
}"#;

pub struct PreparedPresentationPlay {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub lowered: LoweredPlanFragment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    Catalog,
    Form,
    Placement,
    Plan,
    Lowering,
}

impl PreparationError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "presentation-catalog-invalid",
            Self::Form => "presentation-form-rejected",
            Self::Placement => "presentation-placement-rejected",
            Self::Plan => "presentation-plan-rejected",
            Self::Lowering => "presentation-lowering-rejected",
        }
    }
}

pub fn prepare(host: &str, boot: &str) -> Result<PreparedPresentationPlay, PreparationError> {
    let catalog = catalog()?;
    let form = conduit_form::parse(FORM_SOURCE, &catalog).map_err(|_| PreparationError::Form)?;
    let advertisement = advertisement(host, boot);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| PreparationError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::Plan);
    }
    let lowered =
        lower_plan_fragment(&plan.fragments[0]).map_err(|_| PreparationError::Lowering)?;
    if !lowered.remote_endpoints.is_empty() {
        return Err(PreparationError::Lowering);
    }
    Ok(PreparedPresentationPlay {
        advertisement,
        plan,
        lowered,
    })
}

fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
    let mut capabilities = conduit_std_catalog::conduitos_presentation_nucleus_offers();
    capabilities.push(text_source_offer());
    capabilities.push(sink_offer(
        DISPLAY_KIND,
        GRAPHICS_SCENE_KIND,
        MAX_GRAPHICS_SCENE_BYTES as u32,
    ));
    capabilities.push(sink_offer(
        LAYOUT_SINK_KIND,
        LAYOUT_FRAME_KIND,
        conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
    ));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: vec![resource_offer(
            &format!("{host}/display"),
            PRESENTATION_RESOURCE_CLASS,
            16,
        )],
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

fn text_source_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-text-source@1"),
        kind_id: kind_id(TEXT_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from("conduitos.fixture/text-source@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_PRESENTATION_PROFILE,
            ),
            implementation_id: ImplementationId::from("conduitos.fixture/text-source@1"),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_PRESENTATION_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("text"),
            value_kind: kind_id(conduit_std_catalog::TEXT_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_std_catalog::MAX_TEXT_BYTES,
        },
    }
}

fn sink_offer(kind: &str, value_kind: &str, maximum_bytes: u32) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("{kind}-capability@1").as_str()),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from("conduitos.fixture/display-sink@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_PRESENTATION_PROFILE,
            ),
            implementation_id: ImplementationId::from(format!("{kind}-implementation@1").as_str()),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_PRESENTATION_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: kind_id(value_kind),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(DISPLAY_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: maximum_bytes,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: maximum_bytes,
        },
    }
}

fn catalog() -> Result<conduit_form::ProfileCatalog, PreparationError> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut catalog = conduit_std_catalog::standard_profile_catalog();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_layout_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_presentation_composition_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PreparationError::Catalog)?;
    conduit_std_catalog::install_graphics_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PreparationError::Catalog)?;
    for (kind, value_kind) in [
        (DISPLAY_KIND, GRAPHICS_SCENE_KIND),
        (LAYOUT_SINK_KIND, LAYOUT_FRAME_KIND),
    ] {
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(
                    "conduitos.fixture/display-sink@1",
                ),
                inputs: vec![PortDescriptor {
                    port_id: port_id("input"),
                    value_kind: kind_id(value_kind),
                    direction: PortDirection::Input,
                    temporal: PortTemporal::Value,
                }],
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .map_err(|_| PreparationError::Catalog)?;
    }
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(TEXT_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from("conduitos.fixture/text-source@1"),
            inputs: Vec::new(),
            outputs: text_source_offer().outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| PreparationError::Catalog)?;
    Ok(catalog)
}
