//! Typed provider protocol simulation. No document reads or platform printing.
use conduit_core::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_plot::{check_syntax_document, expand_canonical_plot, parse_syntax_document};
use conduit_process::*;
use std::collections::BTreeMap;

#[allow(dead_code)]
#[path = "local_model_pool.rs"]
mod generic;

pub fn identity(s: &str) -> PrintIdentity {
    PrintIdentity::new(s.into()).unwrap()
}
pub fn binding() -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: ResourceSemanticIdentity::from_digest([7; 32]),
        version: ResourceVersionIdentity::from_digest([8; 32]),
        content_profile: PRINT_DOCUMENT_CONTENT_PROFILE.into(),
        access_class: PRINT_DOCUMENT_ACCESS_CLASS.into(),
        handle: "fixture/document-not-dereferenced".into(),
        authority_contract: PRINT_DOCUMENT_AUTHORITY.into(),
        authority_grant: "fixture/document-authority".into(),
        maximum_bytes: 4096,
        maximum_items: Some(2),
        availability: ResourceReferenceAvailability::Available,
    }
}
pub fn request(color: PrintColor, media: PrintMedia, copies: u16) -> PrintRequest {
    let b = binding();
    let document = BoundedResourceRef {
        identity: b.identity,
        content_profile: b.content_profile,
        access_class: b.access_class,
        extent: ResourceExtent {
            bytes: 4096,
            items: Some(2),
        },
        lifetime: ResourceLifetime {
            version: b.version,
            expires_at: None,
        },
    };
    PrintRequest::new(
        PrintAttributes::new(color, copies, media, PrintSides::OneSided).unwrap(),
        PrintDocument::new(document).unwrap(),
        identity("fixture/job"),
    )
    .unwrap()
}
pub fn support(color: bool) -> PrintSupport {
    PrintSupport::new(
        PrintColorSupport::new(
            BoundedSequence::try_from_iter(if color {
                vec![PrintColor::Monochrome, PrintColor::Color]
            } else {
                vec![PrintColor::Monochrome]
            })
            .unwrap(),
        )
        .unwrap(),
        if color { 10 } else { 2 },
        PrintMediaSupport::new(
            BoundedSequence::try_from_iter([PrintMedia::A4, PrintMedia::Letter]).unwrap(),
        )
        .unwrap(),
        PrintSidesSupport::new(BoundedSequence::try_from_iter([PrintSides::OneSided]).unwrap())
            .unwrap(),
    )
    .unwrap()
}

pub struct PreparedPrinters {
    pub plan: Plan,
    /// Only providers admitted for this exact immutable request class.
    pub hosts: Vec<HostAdvertisement>,
    pub refused: Vec<(HostId, PrintAdmissionRefusal)>,
    pub request: PrintRequest,
    pub demand: [u8; 32],
}
impl PreparedPrinters {
    pub fn prepare(request: PrintRequest) -> Self {
        use sha2::{Digest, Sha256};
        let (mut startup, mut profiles) = generic::catalogs();
        conduit_semantic_catalog::install_print_catalogs(&mut startup, &mut profiles).unwrap();
        // Deliberately not print/submit: this model never executes its Host Call.
        let mut kind = conduit_semantic_catalog::print_submit_semantic_contract();
        kind.kind_id = kind_id("fixture/print-protocol-model");
        kind.kind_contract_revision = "fixture/print-protocol-model@1".into();
        kind.semantic_laws = vec![];
        profiles.insert_kind(kind.clone()).unwrap();
        let mut hosts = vec![];
        let mut refused = vec![];
        for color in [false, true] {
            let name = if color { "color-slow" } else { "mono-fast" };
            let host_id = HostId::from(format!("fixture/{name}"));
            if let Err(reason) = admit_print_request(&request, &support(color), &binding()) {
                refused.push((host_id, reason));
                continue;
            }
            hosts.push(HostAdvertisement {
                protocol_version: PROTOCOL_VERSION, host_id,
                boot_id: format!("fixture/{name}/boot").into(), offer_generation: OfferGeneration(1),
                profile: "fixture/typed-print-model@1".into(), bases: vec![],
                resources: vec![resource_offer(&format!("fixture/{name}/slot"), "fixture/print-slot", 1)],
                capabilities: vec![conduit_core::capability_offer_from_parts! {
                    semantic_contract: kind.semantic_contract(),
                    startup_parameters: vec![], shorthand: None,
                    capability_id: format!("fixture/{name}/model").into(),
                    kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
                    implementation: ImplementationOffer {
                        execution_profile_id: "fixture/typed-print-model@1".into(),
                        implementation_id: format!("fixture/{name}/model@1").into(),
                        artifact_id: "fixture/print-protocol-model@1".into(),
                    },
                    inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
                    host_calls: vec![], resource_requirements: vec![ResourceRequirement {
                        class_id: "fixture/print-slot".into(), units: 1, protected_role: None, compute: None, content: None,
                    }], authority_requirements: vec![],
                    limits: kind.limits.clone(),
                }], planner_capabilities: vec![],
            });
        }
        assert!(
            !hosts.is_empty(),
            "no eligible provider; retain refusals instead of preparing a pool"
        );
        let source = format!(
            r#"
plot fixture/print-protocol-model (
    request: PrintRequest >> lifecycle: PrintLifecycle
) {{
}}
plot consumer (
    workers: Pool
) {{
    observe: flow/pool-observe(workers)
}}
plot fixture-print-service {{
 pool workers: fixture/print-protocol-model(size = {})
 client: consumer(workers)
}}
"#,
            hosts.len()
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_plot(&checked, "fixture-print-service", &profiles).unwrap();
        assert_eq!(
            expanded.shared_pools[0].member_front,
            hosts[0].capabilities[0].checked_front()
        );
        assert!(
            hosts[0].capabilities[0].limits.max_queue_bytes >= 8192,
            "{}",
            hosts[0].capabilities[0].limits.max_queue_bytes
        );
        let mut all = vec![generic::consumer_host(&expanded.gears[0].checked_front())];
        all.extend(hosts.iter().cloned());
        let placements = conduit_planner::default_expanded_placements(&expanded, &all).unwrap();
        let pool_id = expanded.shared_pools[0].pool_id.clone();
        let mut authority = generic::authority();
        authority.grant_id = "grant/fixture-print-pool".into();
        let requirements = BTreeMap::from([(
            pool_id.clone(),
            conduit_planner::SharedPoolPlanningRequirement {
                member_limits: PoolMemberLimits {
                    queue_item_capacity: 1,
                    queue_byte_capacity: 8192,
                    sign_item_capacity: 8,
                    sign_byte_capacity: 8192,
                },
                admission_authority: authority,
                member_sessions_required: false,
            },
        )]);
        let plan = conduit_planner::plan_expanded_canonical_with_shared_pool_policies(
            &expanded,
            &all,
            &placements,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 8192,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &requirements,
            &[conduit_planner::SharedPoolPolicySelection {
                pool_id: &pool_id,
                policy: SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
            }],
        )
        .unwrap();
        assert!(verify_plan(&plan));
        let demand = Sha256::digest(request.clone().encode().unwrap()).into();
        // Costs and observations use the sealed envelope's order, never advertisement order.
        hosts.sort_by_key(|h| {
            plan.fragments[0].shared_pools[0]
                .realization_envelope
                .iter()
                .position(|r| r.host_id == h.host_id)
                .unwrap()
        });
        Self {
            plan,
            hosts,
            refused,
            request,
            demand,
        }
    }
    pub fn observations(&self, down: Option<usize>) -> Vec<PoolRealizationObservation> {
        self.plan.fragments[0].shared_pools[0]
            .realization_envelope
            .iter()
            .enumerate()
            .map(|(i, r)| PoolRealizationObservation {
                host_id: r.host_id.clone(),
                boot_id: r.boot_id.clone(),
                offer_generation: r.offer_generation,
                capability_id: r.capability_id.clone(),
                implementation_id: r.implementation_id.clone(),
                artifact_id: r.artifact_id.clone(),
                health: if down == Some(i) {
                    PoolRealizationHealth::Unavailable
                } else {
                    PoolRealizationHealth::Ready
                },
                sign_id: format!("fixture/provider-observation/{i}").into(),
                resources: r
                    .resources
                    .iter()
                    .map(|resource| ResourceObservation {
                        host_id: r.host_id.clone(),
                        boot_id: r.boot_id.clone(),
                        offer_generation: r.offer_generation,
                        pool_id: resource.pool_id.clone(),
                        class_id: resource.class_id.clone(),
                        health: ResourceHealth::Ready,
                        unreserved_units: 1,
                        utilized_units: 0,
                        sign_id: format!("fixture/slot-observation/{i}").into(),
                    })
                    .collect(),
            })
            .collect()
    }
    pub fn costs<'a>(
        &'a self,
        operation: &PoolOperationId,
        mono_queue_ms: i128,
    ) -> Vec<PreparedPoolCost<'a>> {
        self.hosts
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let milliseconds = if h.host_id.as_str().contains("mono-fast") {
                    10 + mono_queue_ms
                } else {
                    100
                };
                PreparedPoolCost::prepare(
                    PoolCostPreparation {
                        plan: &self.plan,
                        fragment_index: 0,
                        pool: &self.plan.fragments[0].shared_pools[0].pool_id,
                        operation,
                        demand: self.demand,
                        realization: i as u16,
                        host: h,
                        offer: &h.capabilities[0],
                        resource: None,
                        calibration: &revision(),
                        expected_execution: None,
                        sign: &format!("fixture/cost/{i}").into(),
                        supporting_signs: &[],
                        observed: &instant(h, 10),
                        expires: &instant(h, 20),
                        evaluated_at: &instant(h, 15),
                        duration: quantity(milliseconds),
                        tolerance: quantity(0),
                    },
                    &mut ExactNumericWork::new(1_000_000_000),
                )
                .unwrap()
            })
            .collect()
    }
}
pub fn revision() -> PoolCostCalibrationRevision {
    PoolCostCalibrationRevision::new("fixture/print-calibration".into(), 1).unwrap()
}
pub fn quantity(ms: i128) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::from_i128(ms),
        Unit::resolve("ms").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap()
}
pub fn instant(host: &HostAdvertisement, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            "fixture/clock".into(),
            TemporalScale::Milliseconds,
            1,
            0,
        )
        .unwrap(),
    )
    .unwrap()
}
