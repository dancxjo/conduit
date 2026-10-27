//! The production Root issuer against real verified renderer preparation and a
//! canonical started Body; pins remain deterministic CPU providers.
use super::*;
use conduit_body as b;
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
#[allow(unexpected_cfgs, dead_code)] // Included target cfg and compatible rich API are exercised separately.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_mask_lease.rs"]
pub(crate) mod production;
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
    start_at(body, host, boot, 1)
}
fn start_at(
    body: &mut b::SealedBodyLifecycleSession,
    host: &c::HostId,
    boot: &c::BootId,
    sequence: u64,
) {
    let current = body.realization().unwrap();
    let play = b::BodyPlayIdentity::bind(&current.plan, sequence);
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
    prepare_at(ready, planned, 0)
}
fn prepare_at(
    ready: &ReadyRenderer<native::Provider>,
    planned: &f::PlannedMaskPlot,
    sequence_base: u64,
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
        last_play_sequence: sequence_base,
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
    let active = c::bind_active_play(
        verified.plan_id(),
        &issuer.host,
        &issuer.boot,
        sequence_base + 1,
    );
    let gear = planned.show_placement();
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let material = conduit_environment_make::SevenSegmentMaskMaterial::prepare(&source).unwrap();
    // SAFETY: exact checked Make image assembled above, not incoming metadata.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            material.planning_image(),
        )
    }
    .unwrap();
    let configuration =
        RendererSourceConfiguration::prepare(&image, &source.role.plot_identity, &gear.gear_id)
            .unwrap();
    let (leases, capsule, guard) = issuer
        .prepare_mask_leases_direct(
            ready,
            &configuration,
            DirectRendererSelection {
                plan: &verified,
                fragment_id: &fragment,
                active: &active,
                placement_id: &gear.placement_id,
                fore: &fore,
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

#[test]
fn complete_native_mask_return_requires_all_19200_accepted_children_and_current_body() {
    let values = Box::leak(Box::new(
        conduit_kernel::FixedValueStore::<2, 8192>::new(16384).unwrap(),
    ));
    let _ = run_complete_native_mask(values, 0, None);
}
#[test]
fn returned_mask_store_supports_two_complete_16_hour_plays_without_reviving_old_refs() {
    let values = Box::leak(Box::new(
        conduit_kernel::FixedValueStore::<2, 8192>::new(16384).unwrap(),
    ));
    let (values, stale) = run_complete_native_mask(values, 0, None);
    let (values, _) = run_complete_native_mask(values, 19200, Some(stale));
    use conduit_kernel::ValueStorage;
    assert_eq!(values.used_items(), 0);
    assert!(values
        .resolve_identity(stale.slot, stale.generation)
        .is_err());
    let first = values.store(&[0]).unwrap();
    let second = values.store(&[0]).unwrap();
    assert!(first.generation > 76_800);
    assert!(second.generation > 38_400);
    println!("native_two_mask_plays children=38400 first_next_generation={} second_next_generation={} stale_refused=true physical=false",first.generation,second.generation);
    values.release(first).unwrap();
    values.release(second).unwrap();
}
fn run_complete_native_mask(
    values: &'static mut conduit_kernel::FixedValueStore<2, 8192>,
    sequence_base: u64,
    prior_stale: Option<conduit_kernel::ValueRef>,
) -> (
    &'static mut conduit_kernel::FixedValueStore<2, 8192>,
    conduit_kernel::ValueRef,
) {
    use crate::{mask_session as ms, native_display::EvidenceSink};
    use b::BodyPlanEvidence;
    use conduit_environment_runtime::mask_kernel::PreparedMaskKernel;
    use conduit_kernel::{self as k, scheduler::SchedulerStatus, ValueStorage};
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let (mut root, leases, capsule, guard) = prepare_at(&ready, &planned, sequence_base);
    let mut body = new_body(&planned);
    start_at(&mut body, &root.host, &root.boot, sequence_base + 1);
    let mut metadata = planned.plan.clone();
    let mut configs = std::collections::BTreeMap::new();
    for gear in &mut metadata.fragments[0].placements {
        configs.insert(
            gear.placement_id.clone(),
            core::mem::take(&mut gear.configuration),
        );
    }
    let verified = c::verify_plan_metadata_with_configurations(metadata, |g| {
        Ok::<_, ()>(configs[&g.placement_id].clone())
    })
    .unwrap();
    let current = body.realization().unwrap();
    let routes = f::AdmittedMaskPlotRoutes::from_sealed_session(
        &body,
        &planned.mask,
        &verified,
        vec![f::SealedMaskPlotRoute {
            route_id: "route/completed/native".into(),
            mask_plot: planned.mask.plot_identity.clone(),
            plan_id: current.plan.plan_id().clone(),
            child_mask_plan_id: None,
            owner_route_seal_id: None,
            placement_ids: verified.preparation_metadata().fragments[0]
                .placements
                .iter()
                .map(|g| g.placement_id.clone())
                .collect(),
            currently_available: true,
        }],
    )
    .unwrap();
    let wardrobe = f::MaskWardrobeControl::new_from_admitted_routes(
        f::BodyMaskWardrobe::new(
            routes.body_id().clone(),
            Some(current.plan.wake_id().clone()),
            f::MaskWardrobe::new(
                f::MaskWardrobeLifetime::Wake,
                vec![planned.mask.plot_identity.clone()],
                vec![planned.mask.plot_identity.clone()],
            )
            .unwrap(),
        )
        .unwrap(),
        &routes,
        None,
    )
    .unwrap();
    let initial = c::bind_active_play(
        &planned.plan.plan_id,
        &root.host,
        &root.boot,
        sequence_base + 1,
    );
    let context = f::PreparedMaskShowContext::prepare(&planned, 8192).unwrap();
    let kernel = PreparedMaskKernel::prepare_parts(&planned)
        .unwrap()
        .finish_with_fixed_storage(values)
        .unwrap();
    let face_owner = Box::leak(Box::new(
        conduit_environment_runtime::publication_face_custody::LocalPublicationFace::new(
            conduit_environment_runtime::retained_publication_face::RetainedPublicationFace::prepare(
                fixture::face().basis, 960, 120000).unwrap())));
    let (face_publisher, face_reader) = face_owner.claim().unwrap();
    let reader = face_reader.lease();
    let mut owner = Some(ready);
    let mut input = ms::Preparation {
        context,
        kernel,
        invocation: capsule,
        routes,
        wardrobe,
        producer: ms::FaceScope {
            producer: planned.mask.plot_identity.clone(),
            active: initial.clone(),
            wake: current.wake.wake_id.clone(),
        },
        reader,
    };
    let fixed_address = &**input.kernel.kernel.values() as *const _ as usize;
    let original_face = input.kernel.face.clone();
    input.kernel.face.cord = conduit_kernel::CordId(u16::MAX);
    let (mut input, reason) = ms::NativeMaskSession::prepare_retained(&mut owner, input)
        .err()
        .expect("deep owned-boundary refusal must retain actual kernel and fixedref");
    assert_eq!(reason, "mask-session");
    assert!(owner.is_some());
    assert_eq!(
        &**input.kernel.kernel.values() as *const _ as usize,
        fixed_address
    );
    assert_eq!(pins.edges.get(), 0);
    input.kernel.face = original_face;
    let mut session = ms::NativeMaskSession::prepare_retained(&mut owner, input)
        .ok()
        .expect("same returned actual renderer and repaired original input");
    let mut leases = Box::new(leases);
    let mut evidence = Box::new(crate::native_mask_evidence::Consumer::new());
    assert!(session.fresh_for_task());
    assert!(leases.fresh_for_task());
    leases.activate(&body, 1000).unwrap();
    let face = fixture::face();
    let mut stale = None;
    for index in 0..19200 {
        let now = 1000 + index * 3000;
        let (active, table, handle, claim) =
            leases.next(session.ready.as_ref().unwrap(), now).unwrap();
        session.call = Some(
            session
                .invocation
                .bind_retained(&mut session.ready, table, handle, claim, &active)
                .unwrap(),
        );
        let face_fore = session.face.clone();
        let show_fore = session.show.clone();
        let mut presentation = None;
        session
            .session
            .admit_owned_input_encoded(face_fore.endpoint, face_fore.cord, index, 8192, |out| {
                let length = serde_json_core::to_slice(&face, out).unwrap();
                presentation = Some(
                    session
                        .call
                        .as_mut()
                        .unwrap()
                        .prepare_presentation(PresentationBasis {
                            face: &face,
                            face_wire: &out[..length],
                            context: &session.context,
                            elapsed_ms: index * 3000,
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
                        .unwrap(),
                );
                Ok::<_, ()>(length)
            })
            .unwrap();
        let mut presentation = presentation.unwrap();
        session
            .session
            .close_owned_input(face_fore.endpoint, face_fore.cord)
            .unwrap();
        let mut emitted = false;
        let mut drained = false;
        for _ in 0..96 {
            let status = session.session.step().unwrap();
            if let Some(request) = session.session.next_host_request() {
                stale.get_or_insert(request.input.value);
                if let Some(old) = prior_stale {
                    assert!(session
                        .session
                        .kernel()
                        .values()
                        .resolve_identity(old.slot, old.generation)
                        .is_err());
                }
                let call = session.call.as_mut().unwrap();
                let verified = call
                    .verify_input(
                        request.node,
                        request.call,
                        request.request,
                        session
                            .session
                            .kernel()
                            .host_value(request.input.value)
                            .unwrap(),
                        &presentation,
                    )
                    .unwrap();
                let output = session
                    .session
                    .store_host_value_encoded(8192, |out| {
                        call.present_verified(
                            request.node,
                            request.call,
                            request.request,
                            verified,
                            out,
                            encode,
                            || false,
                        )
                    })
                    .unwrap();
                session
                    .session
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output: Some(k::BoundedValueRef::new(output, 8192).unwrap()),
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
            if let Some(offer) = session
                .session
                .owned_output(show_fore.endpoint, show_fore.cord)
                .unwrap()
            {
                assert!(!emitted);
                session
                    .session
                    .accept_owned_output(offer.endpoint, offer.cord, offer.sequence)
                    .unwrap();
                emitted = true;
            }
        }
        assert!(drained && emitted);
        session.call.as_mut().unwrap().withdraw(|| false).unwrap();
        let closed = session
            .session
            .store_host_value_encoded(8192, |out| {
                session.call.as_ref().unwrap().closed_encoded(
                    &mut presentation,
                    c::bind_sign(
                        &active.host_id,
                        &active.boot_id,
                        Some(&active.active_play_id),
                        2,
                    )
                    .sign_id,
                    out,
                    encode,
                )
            })
            .unwrap();
        drop(presentation);
        let inspection = session.call.as_ref().unwrap().inspection().clone();
        evidence
            .accept(
                index,
                &active,
                session.session.kernel().host_value(closed).unwrap(),
                session.session.kernel().signs(),
            )
            .unwrap();
        session.ready = Some(
            session
                .invocation
                .retire_invocation(&mut session.call)
                .unwrap(),
        );
        leases
            .validate_retired(session.ready.as_ref().unwrap(), &active, &inspection)
            .unwrap();
        session.session.discard_host_value(closed).unwrap();
        session.index += 1;
        if index == 0 {
            let addresses = (
                &*session as *const _ as usize,
                &*leases as *const _ as usize,
                &*evidence as *const _ as usize,
            );
            let (s, l, e, _) = session.retire_completed(leases, evidence).err().unwrap();
            session = s;
            leases = l;
            evidence = e;
            assert_eq!(
                addresses,
                (
                    &*session as *const _ as usize,
                    &*leases as *const _ as usize,
                    &*evidence as *const _ as usize
                )
            );
        }
        if index + 1 < 19200 {
            let _accepted_previous = session
                .session
                .begin_next([(); 3], ms::NativeMaskSession::fresh_log().unwrap())
                .unwrap_or_else(|_| panic!("exact finite child rearm"));
        }
    }
    assert_eq!(evidence.completion(), (19200, Some(sequence_base + 19200)));
    assert!(session.session.retirement_ready());
    let (s, l, e, _) = session
        .retire_completed(leases, evidence)
        .err()
        .expect("live witness must refuse even at exact count");
    session = s;
    leases = l;
    evidence = e;
    guard.revoke();
    let evidence_address = &*evidence as *const _ as usize;
    let session_address = &*session as *const _ as usize;
    for failure in 0..3 {
        match failure {
            0 => session.faulted = true,
            1 => session.cleanup_refusal = Some("fixture-refusal"),
            _ => {
                session.pending = Some((
                    initial.clone(),
                    k::ValueRef {
                        slot: 0,
                        generation: 0,
                        byte_len: 1,
                    },
                ))
            }
        }
        let (s, l, e, _) = session.retire_completed(leases, evidence).err().unwrap();
        session = s;
        leases = l;
        evidence = e;
        assert_eq!(&*session as *const _ as usize, session_address);
        assert_eq!(&*evidence as *const _ as usize, evidence_address);
        session.faulted = false;
        session.cleanup_refusal = None;
        session.pending = None;
    }
    let fresh = Box::new(crate::native_mask_evidence::Consumer::new());
    let (s, l, fresh, _) = session
        .retire_completed(leases, fresh)
        .err()
        .expect("unaccepted evidence must refuse");
    session = s;
    leases = l;
    drop(fresh);
    let (retired, memory) =
        allocation_probe::observe(|| session.retire_completed(leases, evidence).ok().unwrap());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(root.last_play_sequence, sequence_base + 19200);
    let generation = root.display_generation;
    let wrong = c::bind_active_play(&initial.plan_id, &root.host, &root.boot, sequence_base + 2);
    assert!(root.finish_mask_play(&body, &wrong, &retired).is_err());
    assert!(root
        .finish_mask_play(&new_body(&planned), &initial, &retired)
        .is_err());
    root.display_generation += 1;
    assert!(root.finish_mask_play(&body, &initial, &retired).is_err());
    root.display_generation = generation;
    assert!(root.display_issued);
    root.finish_mask_play(&body, &initial, &retired).unwrap();
    assert!(!root.display_issued);
    assert_eq!(root.last_play_sequence, sequence_base + 19200);
    assert_eq!(root.display_generation, generation);
    assert!(root.finish_mask_play(&body, &initial, &retired).is_err());
    let returned = retired.into_parts();
    assert_eq!(&*returned.evidence as *const _ as usize, evidence_address);
    assert!(!returned.kernel.cancelled);
    assert_eq!(returned.kernel.active_nodes, 3);
    assert!(returned.reader.with_retained(|_, _| ()).is_ok());
    let _returned_face = face_owner
        .reunite(face_publisher, face_reader)
        .ok()
        .unwrap();
    assert!(matches!(
        returned.reader.with_retained(|_, _| ()),
        Err(conduit_environment_runtime::publication_face_custody::CustodyRefusal::Scope)
    ));
    assert_eq!(pins.released.get(), [true, true]);
    assert!(pins.edges.get() > 19200 * 200);
    println!("native_completed_mask_cpu children=19200 completeClosed=19200 retirement_allocations=0 physical=false");
    (returned.kernel.values, stale.unwrap())
}
