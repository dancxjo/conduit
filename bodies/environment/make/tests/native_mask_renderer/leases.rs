//! The production Root issuer against real verified renderer preparation and a
//! canonical started Body; pins remain deterministic CPU providers.
use super::*;
use conduit_body as b;
use conduit_plan_lowering::selected_operation::*;
struct RootIssuer {
    host: c::HostId,
    boot: c::BootId,
    key: [u8; 32],
    display_issued: bool,
    reserved_display_play: Option<Box<c::ActivePlayIdentity>>,
    last_play_sequence: u64,
    display_generation: u64,
    issued_display_play: Option<Box<c::ActivePlayIdentity>>,
}
impl RootIssuer {
    fn next_play_sequence(&self) -> Option<u64> {
        self.last_play_sequence.checked_add(1)
    }
    fn grant_mask(&self, native: &RendererIdentity) -> Result<c::AuthorityGrant, ()> {
        if native.host_id != self.host || native.boot_id != self.boot {
            return Err(());
        }
        Ok(c::AuthorityGrant {
            grant_id: "grant/fixture/render".into(),
            contract_id: AUTHORITY.into(),
            host_call_contract_id: CALL.into(),
            subject_kind: f::RENDERER_KIND.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: CAPABILITY.into(),
        })
    }
}
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_mask_lease.rs"]
mod production;
fn new_body(planned: &f::PlannedMaskPlot) -> b::SealedBodyLifecycleSession {
    let fragment = &planned.plan.fragments[0];
    let host = &fragment.host_id;
    let boot = &fragment.boot_id;
    let resident = b::ResidentPlot::new(
        planned.mask.plot_identity.source_document_id.clone(),
        planned.mask.plot_identity.checked_plot_id.clone(),
    );
    let body = b::Body::born_with_plots(
        b::BodyWorkset::from_plots(vec![resident.clone()]).unwrap(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        b::BodyBiographyEvidence::born(body.clone(), membership.clone(), "Mask leases".into())
            .unwrap();
    let part = b::PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = b::MembershipProofId::bind("native-mask-local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "admit".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            b::AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: fragment.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            "present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let mut session = b::SealedBodyLifecycleSession::open_admitted(evidence, host, boot).unwrap();
    session
        .propose_with_masks(
            vec![b::BodyPlotPlan {
                plot: resident,
                plan: planned.plan.clone(),
            }],
            vec![b::BodyMaskTopology {
                face: b::BodyFaceSelector {
                    plot: None,
                    source_placement_id: None,
                },
                chains: vec![b::BodyMaskChainPlan {
                    plan: planned.plan.clone(),
                    stage_placement_ids: vec![fragment.placements[0].placement_id.clone()],
                }],
            }],
            host,
            boot,
        )
        .unwrap();
    session
}
fn start(body: &mut b::SealedBodyLifecycleSession, host: &c::HostId, boot: &c::BootId) {
    let current = body.realization().unwrap();
    let play = b::BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |sequence| c::bind_sign(host, boot, Some(&play.active_play_id), sequence).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .and_then(|wake| wake.body_play_started(&current.plan, &play, sign(1)))
        .unwrap();
    body.started(host, boot, play, wake).unwrap();
}
fn prepare(
    ready: &ReadyRenderer<native::Provider>,
    planned: &f::PlannedMaskPlot,
) -> (
    RootIssuer,
    production::MaskLeaseIssuer,
    PreparedRendererInvocation,
    production::MaskScopeGuard,
) {
    let mut issuer = RootIssuer {
        host: ready.identity().host_id.clone(),
        boot: ready.identity().boot_id.clone(),
        key: [91; 32],
        display_issued: false,
        reserved_display_play: None,
        last_play_sequence: 0,
        display_generation: 0,
        issued_display_play: None,
    };
    let mut metadata = planned.plan.clone();
    let fragment = metadata.fragments[0].fragment_id.clone();
    let fore = metadata.fragments[0].fore_ports.clone();
    let configurations: std::collections::BTreeMap<_, _> = metadata
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .map(|gear| {
            (
                gear.placement_id.clone(),
                core::mem::take(&mut gear.configuration),
            )
        })
        .collect();
    let verified = c::verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(configurations[&gear.placement_id].clone())
    })
    .unwrap();
    let lowering = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
        &verified, &fragment,
    )
    .unwrap();
    let active = c::bind_active_play(verified.plan_id(), &issuer.host, &issuer.boot, 1);
    let gear = planned.show_placement();
    let (leases, capsule, guard) = issuer
        .prepare_mask_leases(
            ready,
            &gear.configuration,
            SelectedVerifiedForeOperationPlan {
                fore: &fore,
                operation: SelectedVerifiedOperationPlan {
                    plan: &verified,
                    fragment_id: &fragment,
                    lowered: &lowering,
                    active: &active,
                    placement_id: &gear.placement_id,
                },
            },
        )
        .unwrap();
    (issuer, leases, capsule, guard)
}
#[test]
fn real_root_issuer_requires_started_body_and_exact_retirement_before_next_child() {
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let (root, mut leases, mut capsule, _guard) = prepare(&ready, &planned);
    let mut body = new_body(&planned);
    assert!(leases.next(&ready, 1000).is_err());
    assert!(leases.activate(&body, 1000).is_err());
    start(&mut body, &root.host, &root.boot);
    leases.activate(&body, 1000).unwrap();
    assert!(leases.activate(&body, 1000).is_err());
    assert!(root.display_issued);
    assert_eq!(root.last_play_sequence, 19200);
    assert_eq!(root.next_play_sequence(), Some(19201));
    let (active, table, handle, claim) = leases.next(&ready, 1000).unwrap();
    assert_eq!(active.play_sequence, 1);
    assert!(leases.next(&ready, 4000).is_err());
    let mut owner = Some(ready);
    let mut call = Some(
        capsule
            .bind_retained(&mut owner, table, handle, claim, &active)
            .unwrap(),
    );
    let face = fixture::face();
    let wire = serde_json::to_vec(&face).unwrap();
    let context = f::PreparedMaskShowContext::prepare(&planned, 8192).unwrap();
    let mut presentation = call
        .as_mut()
        .unwrap()
        .prepare_presentation(PresentationBasis {
            face: &face,
            face_wire: &wire,
            context: &context,
            elapsed_ms: 0,
            front: "environment".into(),
            target: "fixture/display".into(),
            prepared_sign: c::bind_sign(
                &active.host_id,
                &active.boot_id,
                Some(&active.active_play_id),
                0,
            )
            .sign_id,
            available_sign: c::bind_sign(
                &active.host_id,
                &active.boot_id,
                Some(&active.active_play_id),
                1,
            )
            .sign_id,
        })
        .unwrap();
    let mut out = vec![0; 8192];
    call.as_mut()
        .unwrap()
        .present_encoded(
            capsule.node(),
            HostCallId(0),
            RequestId(0),
            &wire,
            &presentation,
            &mut out,
            encode,
            || false,
        )
        .unwrap();
    let spent = call.as_ref().unwrap().inspection().clone();
    assert_eq!(spent.completed_operations, 1);
    assert!(leases
        .validate_retired(
            &native::ready(Rc::new(native::Pins::default())),
            &active,
            &spent
        )
        .is_err());
    call.as_mut().unwrap().withdraw(|| false).unwrap();
    let inspection = call.as_ref().unwrap().inspection().clone();
    assert_eq!(inspection.completed_operations, 2);
    assert_eq!(inspection.lifecycle, c::CapabilityLifecycle::Revoked);
    call.as_ref()
        .unwrap()
        .closed_encoded(
            &mut presentation,
            c::bind_sign(
                &active.host_id,
                &active.boot_id,
                Some(&active.active_play_id),
                2,
            )
            .sign_id,
            &mut out,
            encode,
        )
        .unwrap();
    let ready = capsule.retire_invocation(&mut call).unwrap();
    leases
        .validate_retired(&ready, &active, &inspection)
        .unwrap();
    assert!(leases
        .validate_retired(&ready, &active, &inspection)
        .is_err());
    assert!(leases.next(&ready, 3999).is_err());
    let (next, _, _, _) = leases.next(&ready, 4000).unwrap();
    assert_eq!(next.play_sequence, 2);
    assert_ne!(next.active_play_id, active.active_play_id);
    assert!(pins.edges.get() > 0);
}
#[test]
fn parent_deadline_revocation_and_global_child_range_are_finite() {
    let pins = Rc::new(native::Pins::default());
    let mut ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let (root, mut leases, mut capsule, guard) = prepare(&ready, &planned);
    let mut body = new_body(&planned);
    start(&mut body, &root.host, &root.boot);
    leases.activate(&body, 0).unwrap();
    for sequence in 1..=19200u64 {
        let (active, table, handle, claim) = leases.next(&ready, (sequence - 1) * 3000).unwrap();
        assert_eq!(active.play_sequence, sequence);
        let mut owner = Some(ready);
        let mut call = Some(
            capsule
                .bind_retained(&mut owner, table, handle, claim, &active)
                .unwrap(),
        );
        call.as_mut().unwrap().cancel_before_delivery().unwrap();
        let inspection = call.as_ref().unwrap().inspection().clone();
        assert_eq!(inspection.completed_operations, 0);
        ready = capsule.retire_invocation(&mut call).unwrap();
        leases
            .validate_retired(&ready, &active, &inspection)
            .unwrap();
    }
    assert!(leases.next(&ready, 960 * 60_000).is_err());
    assert_eq!(pins.edges.get(), 0);
    let (root, mut leases, _, guard2) = prepare(&ready, &planned);
    let mut body = new_body(&planned);
    start(&mut body, &root.host, &root.boot);
    leases.activate(&body, 500).unwrap();
    assert!(leases.next(&ready, 500 + 960 * 60_000).is_err());
    let task_guard = leases.revocation_guard();
    task_guard.revoke();
    guard2.revoke();
    assert!(leases.next(&ready, 500).is_err());
    drop(guard);
    assert_eq!(pins.edges.get(), 0);
}
