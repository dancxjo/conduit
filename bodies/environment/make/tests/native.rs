//! Only pulse capture is simulated. Make never creates a Ready provider.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[allow(
    dead_code,
    reason = "shared transcript exercises selected and assigned ownership"
)]
#[path = "../../runtime/tests/support/assignment.rs"]
mod assignment;
mod plan {
    pub const PREFIX: &str = "fixture/environment";
}
#[allow(
    dead_code,
    reason = "shared advertisement also supports rich kernel conformance"
)]
#[path = "../../runtime/tests/appliance_support/mod.rs"]
mod appliance_support;
#[allow(
    dead_code,
    reason = "shared transcript exercises selected and assigned ownership"
)]
#[path = "../../runtime/tests/support/pulse.rs"]
mod pulse;
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_environment_make::{NativeApplianceLimits, PreparedNativeAppliance};
use conduit_environment_runtime::{
    assigned::{AssignedKernelTables, ResidentLayout},
    construction,
};
const N: usize = 15;
const C: usize = 14;
const P: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn limits() -> NativeApplianceLimits {
    NativeApplianceLimits {
        image: e::EmbeddedImageBounds {
            maximum_nodes: N,
            maximum_cords: C,
            maximum_routes: N,
            maximum_route_targets: C,
            maximum_host_calls: N,
            maximum_resources: 1,
            maximum_sign_expectations: 128,
            maximum_configuration_entries: 16,
            maximum_ports_per_node: P,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: C as u16,
            maximum_cord_value_bytes: 32768,
            maximum_sign_items: 128,
            maximum_sign_bytes: 32768,
        },
        computations: e::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        assignment: assignment::NativeProjection::maxima(),
    }
}
#[test]
fn real_authored_source_is_prepared_against_supplied_offers_without_effects() {
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let host = appliance_support::advertisement(&source, &ready);
    let bases = ["conduit.base/local@1".into()];
    assert!(PreparedNativeAppliance::prepare(&source, &host, &bases, &[], limits()).is_err());
    let prepared =
        PreparedNativeAppliance::prepare(&source, &host, &bases, &[pulse::grant()], limits())
            .unwrap();
    assert_eq!(effects.get(), 0);
    assert!(c::verify_plan(prepared.plan()));
    assert_eq!(prepared.image().plan_id, prepared.plan().plan_id.as_str());
    assert_eq!(prepared.image().boot_id, host.boot_id.as_str());
    assert_eq!(prepared.image().nodes.len(), N);
    let calls: Vec<_> = prepared
        .image()
        .host_calls
        .iter()
        .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
        .collect();
    let resources: Vec<_> = prepared
        .image()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(host.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(host.boot_id.as_str()),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let profile = c::AssignedComputationProfile {
        maximum_text_bytes: 32768,
        maximum_total_text_bytes: 45000,
        requirements: prepared.computations(),
    };
    let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&prepared.assignment()[80..80 + c::ASSIGNED_PLAN_COUNT_KINDS]);
    let layout = ResidentLayout {
        records: prepared.resident_records(),
        counts,
    };
    let (tables, allocation) = allocation_probe::observe(|| {
        AssignedKernelTables::<N, C, P, { N * P }, C, N>::prepare(
            prepared.assignment(),
            limits().assignment,
            required,
            profile,
            &layout,
        )
        .unwrap()
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(
        tables.nodes,
        core::array::from_fn(|i| prepared.lowered().node_specs[i])
    );
    let stale = c::AssignedPlanRequirements {
        boot: c::AssignedIdentity::from_text("stale"),
        ..required
    };
    assert!(AssignedKernelTables::<N, C, P, { N * P }, C, N>::prepare(
        prepared.assignment(),
        limits().assignment,
        stale,
        profile,
        &layout,
    )
    .is_err());
    let mut small = limits();
    small.assignment.runtime_state_bytes = 1;
    assert!(
        PreparedNativeAppliance::prepare(&source, &host, &bases, &[pulse::grant()], small).is_err()
    );
    assert_eq!(effects.get(), 0);
}

/// CPU fixture membership, not an authenticated physical board or started Play.
fn body_owner(
    prepared: &PreparedNativeAppliance,
    host: &c::HostId,
    boot: &c::BootId,
    wrong_source: bool,
) -> conduit_body::BodyLifecycleSession {
    use conduit_body::*;
    let resident = prepared.body_plot_plan().plot;
    let body = Body::born(
        if wrong_source {
            "fixture/different-source".into()
        } else {
            resident.source_document_id
        },
        resident.checked_plot_id,
        1,
        c::bind_sign(host, boot, None, 1).sign_id,
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 0).unwrap();
    let proof = MembershipProofId::bind("fixture/Make-lifecycle").unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            c::bind_sign(host, boot, None, 2).sign_id,
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: c::OfferGeneration(1),
                proof_id: proof,
                sequence: 0,
            },
            c::bind_sign(host, boot, None, 3).sign_id,
        )
        .unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone()).unwrap(),
        "Mica Quail".into(),
    )
    .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    BodyLifecycleSession::open_admitted(evidence, host, boot).unwrap()
}
#[test]
fn native_construction_enters_the_real_body_proposal_without_inventing_a_play() {
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let host = appliance_support::advertisement(&source, &ready);
    let prepared = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mut owner = body_owner(&prepared, &host.host_id, &host.boot_id, false);
    let before = owner.evidence().clone();
    let proposal = prepared.propose_to_body(&mut owner, vec![]).unwrap();
    assert!(proposal.play.is_none());
    assert_eq!(
        proposal.wake.lifecycle,
        conduit_body::WakeLifecycle::AwaitingPlan
    );
    assert_eq!(proposal.plan.plots[0].plan, *prepared.plan());
    assert_eq!(proposal.plan.plots[0].plot, prepared.body_plot_plan().plot);
    assert_eq!(proposal.wake.body_id, before.body_id);
    assert_eq!(owner.evidence().friendly_name, "Mica Quail");
    assert!(owner.evidence().records.len() > before.records.len());
    owner.evidence().validate().unwrap();
    let retained = owner.evidence().clone();
    assert_eq!(
        prepared.propose_to_body(&mut owner, vec![]).unwrap_err(),
        conduit_body::BodyLifecycleSessionError::NotLulled
    );
    assert_eq!(owner.evidence(), &retained);
    assert_eq!(effects.get(), 0);
}
#[test]
fn stale_boot_or_wrong_workset_cannot_acquire_a_wake_from_native_preparation() {
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let host = appliance_support::advertisement(&source, &ready);
    let prepared = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mut stale = body_owner(&prepared, &host.host_id, &"fixture/old-boot".into(), false);
    let retained = stale.evidence().clone();
    assert_eq!(
        prepared.propose_to_body(&mut stale, vec![]).unwrap_err(),
        conduit_body::BodyLifecycleSessionError::StaleHost
    );
    assert_eq!(stale.evidence(), &retained);
    assert!(stale.realization().is_none());
    let mut wrong = body_owner(&prepared, &host.host_id, &host.boot_id, true);
    let retained = wrong.evidence().clone();
    assert!(matches!(
        prepared.propose_to_body(&mut wrong, vec![]),
        Err(conduit_body::BodyLifecycleSessionError::Plan(_))
    ));
    assert_eq!(wrong.evidence(), &retained);
    assert!(wrong.realization().is_none());
    assert_eq!(effects.get(), 0);
}
