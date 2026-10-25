//! Real checked/sealed Plan fixture; software lifecycle and custody proof only.
use super::*;
use conduit_body::{Body, BodyPlotPlan};
use conduit_body_make::deployment_runtime::CrecheDisposition;
use conduit_body_make::deployment_runtime_catalog::{install_creche_catalogs, CRECHE_RESOURCE};
use conduit_core::{ConnectionTrack, PortDirection};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use std::collections::BTreeMap;

pub(crate) struct Adapter {
    pub(crate) resource: ResourceOffer,
    pub(crate) limits: CrecheLimits,
    pub(crate) request: Option<CrecheRequest>,
    pub(crate) begins: usize,
    pub(crate) polls: usize,
    pub(crate) drains: usize,
    pub(crate) pending: bool,
    pub(crate) corrupt: bool,
    pub(crate) evidence: Vec<u8>,
    pub(crate) stop_on_begin: Option<crate::RunControl>,
    pub(crate) fail_drain: bool,
}
impl Adapter {
    pub(crate) fn new() -> Self {
        Self {
            resource: conduit_core::resource_offer("test/creche-slot", CRECHE_RESOURCE, 1),
            limits: CrecheLimits {
                maximum_evidence_bytes: 1024,
                maximum_polls: 2,
                minimum_poll_interval_ms: 1,
            },
            stop_on_begin: None,
            fail_drain: false,
            request: None,
            begins: 0,
            polls: 0,
            drains: 0,
            pending: false,
            corrupt: false,
            evidence: br#"{"schema":"fixture/creche-classified-evidence@1"}"#.to_vec(),
        }
    }
}
impl HostedCrecheAdapter for Adapter {
    fn resource_offer(&self) -> ResourceOffer {
        self.resource.clone()
    }
    fn limits(&self) -> CrecheLimits {
        self.limits
    }
    fn prepare(&mut self, _: &CrecheExecutionContext) -> Result<PreparedCrecheRequest, String> {
        Ok(PreparedCrecheRequest {
            admitted_state_digest: [7; 32],
        })
    }
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        self.begins += 1;
        self.request = Some(request);
        if let Some(control) = &self.stop_on_begin {
            control
                .request_stop(crate::RunControlRequestId::new("fixture/creche-stop").unwrap())
                .unwrap();
        }
        Ok(CrechePoll::Pending)
    }
    fn poll(&mut self) -> Result<CrechePoll, String> {
        self.polls += 1;
        if self.pending {
            return Ok(CrechePoll::Pending);
        }
        Ok(CrechePoll::Outcome(CrecheOutcome {
            request: self.request.unwrap(),
            disposition: CrecheDisposition::Refused,
            evidence_digest: if self.corrupt {
                [0; 32]
            } else {
                Sha256::digest(&self.evidence).into()
            },
        }))
    }
    fn retained_evidence(&self, _: &CrecheOutcome) -> Result<&[u8], String> {
        Ok(&self.evidence)
    }
    fn cancel_and_drain(&mut self) -> Result<(), String> {
        self.drains += 1;
        if self.fail_drain {
            return Err("fixture drain failure".into());
        }
        Ok(())
    }
}

pub(crate) fn planned(
    adapter: &Adapter,
) -> (
    HostAdvertisement,
    BodyPlan,
    BodyPlayIdentity,
    conduit_body::Wake,
) {
    let host = crate::StdHost::new_with_creche_encounter(
        crate::StdHostConfig {
            host_id: "host/test-creche".into(),
            boot_id: "boot/test-creche".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        crate::StdHostComposition::minimal(),
        adapter.resource_offer(),
    )
    .unwrap()
    .advertisement()
    .clone();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_creche_catalogs(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../../plots/creche/main.conduit");
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "creche/main", &profile)
            .unwrap();
    let hosts = [host.clone()];
    let choices = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "request", 32),
        (PortDirection::Output, "outcome", 65),
    ]
    .into_iter()
    .map(|(direction, name, bytes)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: conduit_core::port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        conduit_core::SignId::from("test/creche-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, conduit_core::SignId::from("test/creche-wake"))
        .unwrap()
        .1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&plan, 0);
    (host, plan, play, wake)
}
