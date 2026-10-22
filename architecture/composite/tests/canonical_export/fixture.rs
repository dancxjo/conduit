//! Generic byte-copy primitive and actual canonical Source; no AI/Language fixture.
use conduit_composite::*;
use conduit_core::*;
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{HostedValueStore, PortId as KernelPortId};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
use conduit_plot::{CheckedSyntaxDocument, ProfileCatalog};
use std::collections::BTreeMap;

pub const ENTRY: &str = "generic-wide-export";
pub const BYTES: u32 = 256;
const IMPLEMENTATION: &str = "test/canonical-byte-copy@1";

pub fn source() -> String {
    format!("plot {ENTRY} (\n >> input: value/bytes <= {BYTES}B\n output: value/bytes <= {BYTES}B >>\n) {{\n first: test/canonical-byte-copy\n second: test/canonical-byte-copy\n input >> first.in\n first.out >> second.in\n second.out >> output\n}}\n")
}
pub struct Fixture {
    pub document: CheckedSyntaxDocument,
    pub catalog: ProfileCatalog,
    pub definition: KernelCompositeDefinition,
}
impl Fixture {
    pub fn new() -> Self {
        let (document, catalog, plan) = Self::prepare_source(&source());
        let definition = rebuild(&document, &catalog, plan).unwrap();
        Self {
            document,
            catalog,
            definition,
        }
    }
    pub fn prepare_source(source: &str) -> (CheckedSyntaxDocument, ProfileCatalog, Plan) {
        let port = |name: &str, direction| PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(BYTES_INFO_ID),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        };
        let inputs = vec![port("in", PortDirection::Input)];
        let outputs = vec![port("out", PortDirection::Output)];
        let contracts = [
            FrontValueLocation::Input(port_id("in")),
            FrontValueLocation::Output(port_id("out")),
        ]
        .into_iter()
        .map(|location| FrontValueContract {
            location,
            contract: CheckedValueContract::new(kind_id(BYTES_INFO_ID), BYTES, vec![]).unwrap(),
        })
        .collect::<Vec<_>>();
        let limits = CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 2,
            max_queue_bytes: BYTES,
        };
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert_kind(Kind {
                kind_id: "test/canonical-byte-copy".into(),
                kind_contract_revision: "test/canonical-byte-copy@1".into(),
                inputs: inputs.clone(),
                outputs: outputs.clone(),
                configuration: vec![],
                startup_parameters: vec![],
                shorthand: None,
                semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts.clone())],
                limits: limits.clone(),
            })
            .unwrap();
        let startup = catalog.startup_catalog().unwrap();
        let document = conduit_plot::check_syntax_document_with_literal_constructors(
            &conduit_plot::parse_syntax_document(source),
            &startup,
            &catalog,
        )
        .unwrap();
        let authoring =
            conduit_plot::expand_canonical_plot_for_authoring(&document, ENTRY, &catalog).unwrap();
        let hosts = [HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "generic-child".into(),
            boot_id: "generic-child-boot".into(),
            offer_generation: OfferGeneration(1),
            profile: "test/generic-byte-copy@1".into(),
            bases: vec![],
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: vec![conduit_core::capability_offer_from_parts! {
                semantic_contract: KindSemanticContract { configuration: vec![], laws: vec![KindSemanticLaw::ValueContracts(contracts)] },
                startup_parameters: vec![], shorthand: None, capability_id: "copy".into(), kind_id: "test/canonical-byte-copy".into(),
                kind_contract_revision: "test/canonical-byte-copy@1".into(),
                implementation: ImplementationOffer { execution_profile_id: "test/generic-byte-copy@1".into(), implementation_id: IMPLEMENTATION.into(), artifact_id: "test/generic-byte-copy-artifact@1".into() },
                inputs, outputs, host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![], limits,
            }],
        }];
        let placements =
            conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).unwrap();
        let empty_bases = BTreeMap::new();
        let empty_lines = BTreeMap::new();
        let bounds = [
            (PortDirection::Input, "input"),
            (PortDirection::Output, "output"),
        ]
        .into_iter()
        .map(|(direction, name)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: port_id(name),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 2,
                    byte_capacity: BYTES,
                },
            )
        })
        .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &authoring,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &empty_bases,
                line_candidates: &empty_lines,
                connection_item_capacity: 2,
                connection_byte_capacity: BYTES,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &bounds,
        )
        .unwrap();
        (document, catalog, plan)
    }
    pub fn rebuild(
        &self,
        plan: Plan,
    ) -> Result<KernelCompositeDefinition, KernelCompositeDefinitionError> {
        rebuild(&self.document, &self.catalog, plan)
    }
}
pub fn rebuild(
    document: &CheckedSyntaxDocument,
    catalog: &ProfileCatalog,
    plan: Plan,
) -> Result<KernelCompositeDefinition, KernelCompositeDefinitionError> {
    KernelCompositeDefinition::from_canonical_export(
        "generic-export".into(),
        "generic-export-boot".into(),
        OfferGeneration(1),
        "test/generic-export@1".into(),
        "test/generic-export@1".into(),
        "test/generic-export-artifact@1".into(),
        document,
        ENTRY,
        catalog,
        plan,
        FailureReason::CompositeCapabilityFailed,
    )
}
pub fn reseal(plan: Plan) -> Plan {
    seal_plan_with_completion(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.fragments,
    )
}
pub fn registry() -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(CopyFactory {
            implementation: IMPLEMENTATION.into(),
        })
        .unwrap();
    registry
}
struct CopyFactory {
    implementation: ImplementationId,
}
impl KernelOperationFactory for CopyFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 0,
            value_bytes: 0,
            maximum_value_bytes: BYTES,
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        _: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        Ok(Box::new(CopyBack))
    }
}
struct CopyBack;
impl StepBack<PORTS> for CopyBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(KernelPortId(0)) {
            if !io.output_ready(KernelPortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume(KernelPortId(0)).is_err() || io.send(KernelPortId(0), value).is_err() {
                return StepOutcome::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidLifecycle,
                    detail: 1,
                });
            }
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
}
