//! Pulse/timer and Boot are CPU fixtures. Make retains actual software owners
//! but never creates a Ready native provider or claims physical installation.
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
    let (_software, host) = retained_advertisement(&source, &ready);
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
    let expected_cadence = (source.count, source.period_ms);
    let recipe = prepared.software_recipe().unwrap();
    let assignment = prepared.assignment().to_vec();
    drop((prepared, source, host, _software));
    let values = conduit_kernel::FixedValueStore::<2, 1024>::new(2048).unwrap();
    let assigned =
        conduit_environment_runtime::appliance::assigned::PreparedAssignedAppliance::prepare(
            &assignment,
            limits().assignment,
            required,
            &recipe.resident(),
            values,
        )
        .unwrap();
    assert_eq!((assigned.count, assigned.period_ms), expected_cadence);
    assert_eq!(assigned.tables.nodes, tables.nodes);
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
    let (_software, host) = retained_advertisement(&source, &ready);
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
    let (_software, host) = retained_advertisement(&source, &ready);
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

/// Simulated Root start receipt for CPU lifecycle proof, not a hardware start.
fn start_body(
    owner: &mut conduit_body::BodyLifecycleSession,
    play: &conduit_body::BodyPlayIdentity,
    host: &c::HostId,
    boot: &c::BootId,
) {
    let current = owner.realization().unwrap();
    let wake = current
        .wake
        .body_plan_ready(
            &current.plan,
            c::bind_sign(host, boot, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &current.plan,
            play,
            c::bind_sign(host, boot, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    owner.started(host, boot, play.clone(), wake).unwrap();
}
#[test]
fn body_and_fragment_plays_are_distinct_and_confirmation_needs_retained_start() {
    use conduit_environment_make::{NativeBodyBinding, NativeBodyBindingRefusal as E};
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let (_software, host) = retained_advertisement(&source, &ready);
    let native = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mut owner = body_owner(&native, &host.host_id, &host.boot_id, false);
    native.propose_to_body(&mut owner, vec![]).unwrap();
    let body_play = conduit_body::BodyPlayIdentity::bind(&owner.realization().unwrap().plan, 1);
    let fragment_play =
        c::bind_active_play(&native.plan().plan_id, &host.host_id, &host.boot_id, 0);
    assert_ne!(body_play.active_play_id, fragment_play.active_play_id);
    let mut binding =
        NativeBodyBinding::prepare(&native, &owner, &body_play, &fragment_play).unwrap();
    assert_eq!(binding.fragment_play(&owner), Err(E::NotStarted));
    assert_eq!(binding.confirm_started(&owner), Err(E::NotStarted));
    start_body(&mut owner, &body_play, &host.host_id, &host.boot_id);
    binding.confirm_started(&owner).unwrap();
    assert_eq!(binding.confirm_started(&owner), Err(E::AlreadyConfirmed));
    let ((), allocated) = allocation_probe::observe(|| {
        assert_eq!(binding.fragment_play(&owner).unwrap(), &fragment_play);
        assert_eq!(binding.body_play(&owner).unwrap(), &body_play);
    });
    assert_eq!((allocated.allocations, allocated.reallocations), (0, 0));
    let projection = assignment::NativeProjection::prepare(native.image());
    // Existing native binder, unchanged: its own fragment identity remains canonical.
    let selected =
        pulse::bind_assigned(ready, native.plan(), native.lowered(), &projection).unwrap();
    assert_eq!(
        selected.0.inspection().scope.active_play_id,
        fragment_play.active_play_id
    );
    assert_eq!(selected.0.inspection().scope.plan_id, native.plan().plan_id);
    assert_eq!(effects.get(), 0);
    owner
        .lull(&host.host_id, &host.boot_id, Some(&body_play))
        .unwrap();
    assert_eq!(binding.fragment_play(&owner), Err(E::StaleProposal));
}
#[test]
fn body_binding_refuses_identity_substitution_membership_loss_and_revocation() {
    use conduit_environment_make::{NativeBodyBinding, NativeBodyBindingRefusal as E};
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let (_software, host) = retained_advertisement(&source, &ready);
    let native = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mut owner = body_owner(&native, &host.host_id, &host.boot_id, false);
    native.propose_to_body(&mut owner, vec![]).unwrap();
    let body_play = conduit_body::BodyPlayIdentity::bind(&owner.realization().unwrap().plan, 1);
    let fragment_play =
        c::bind_active_play(&native.plan().plan_id, &host.host_id, &host.boot_id, 42);
    for substitute in [body_play.active_play_id.clone(), "forged/play".into()] {
        let mut bad = fragment_play.clone();
        bad.active_play_id = substitute;
        assert!(matches!(
            NativeBodyBinding::prepare(&native, &owner, &body_play, &bad),
            Err(E::WrongFragment)
        ));
    }
    let stale = c::bind_active_play(
        &native.plan().plan_id,
        &host.host_id,
        &"fixture/old-boot".into(),
        42,
    );
    assert!(matches!(
        NativeBodyBinding::prepare(&native, &owner, &body_play, &stale),
        Err(E::WrongFragment)
    ));
    let mut bad_body = body_play.clone();
    bad_body.active_play_id = fragment_play.active_play_id.clone();
    assert!(matches!(
        NativeBodyBinding::prepare(&native, &owner, &bad_body, &fragment_play),
        Err(E::StaleProposal)
    ));
    let mut binding =
        NativeBodyBinding::prepare(&native, &owner, &body_play, &fragment_play).unwrap();
    let mut revoked =
        NativeBodyBinding::prepare(&native, &owner, &body_play, &fragment_play).unwrap();
    revoked.revoke();
    assert_eq!(revoked.confirm_started(&owner), Err(E::Revoked));
    start_body(&mut owner, &body_play, &host.host_id, &host.boot_id);
    binding.confirm_started(&owner).unwrap();
    assert_eq!(revoked.fragment_play(&owner), Err(E::Revoked));
    owner
        .observe_host_lost(&host.host_id, &host.boot_id, &host.host_id, &host.boot_id)
        .unwrap();
    assert_eq!(binding.fragment_play(&owner), Err(E::StaleHost));
    assert_eq!(effects.get(), 0);
}

#[test]
fn a_different_retained_body_play_cannot_confirm_the_prepared_binding() {
    use conduit_environment_make::{NativeBodyBinding, NativeBodyBindingRefusal as E};
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let (_software, host) = retained_advertisement(&source, &ready);
    let native = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mut owner = body_owner(&native, &host.host_id, &host.boot_id, false);
    native.propose_to_body(&mut owner, vec![]).unwrap();
    let prepared = conduit_body::BodyPlayIdentity::bind(&owner.realization().unwrap().plan, 1);
    let issued_other = conduit_body::BodyPlayIdentity::bind(&owner.realization().unwrap().plan, 2);
    let fragment = c::bind_active_play(&native.plan().plan_id, &host.host_id, &host.boot_id, 9);
    let mut binding = NativeBodyBinding::prepare(&native, &owner, &prepared, &fragment).unwrap();
    start_body(&mut owner, &issued_other, &host.host_id, &host.boot_id);
    assert_eq!(binding.confirm_started(&owner), Err(E::StalePlay));
    assert_eq!(binding.fragment_play(&owner), Err(E::NotStarted));
    assert_eq!(effects.get(), 0);
}

fn retained_advertisement(
    source: &construction::ApplianceSource,
    ready: &conduit_pulse_capture::installation::ReadyPulseBase<pulse::Provider>,
) -> (
    conduit_environment_runtime::appliance::software::ReadySoftwareBacks,
    c::HostAdvertisement,
) {
    let mut software =
        conduit_environment_runtime::appliance::software::ReadySoftwareBacks::prepare(
            source,
            pulse::identity().host_id,
            pulse::identity().boot_id,
        )
        .unwrap();
    let host = appliance_support::advertisement(source, &mut software, ready);
    (software, host)
}

#[path = "native_support/recipe.rs"]
mod recipe;
