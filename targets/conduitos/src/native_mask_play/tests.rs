extern crate std;
use super::*;
use crate::display::{DisplayError, DisplayFormat, PixelTarget};
use crate::native_compositor::{CompositorAdmission, NativeCompositor};
use conduit_core::{HostBaseId, HostId, SignId};
use conduit_presentation::{
    GraphicsScene, LayoutRect, PresentationBasis, PresentationRole, PresentationSubject,
};

struct FixturePixelTarget {
    writes: u32,
    fail: bool,
}
impl PixelTarget for FixturePixelTarget {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 16,
            height: 16,
            pitch: 64,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        if self.fail {
            return Err(DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
}

fn fixture_scanout(sequence: u64) -> (PreparedNativeMaskPlay, NativeCompositor) {
    let stage = crate::mask_control::prepare_stage(
        crate::mask_control::Adapter::Native,
        &HostId::from("host/native-mask"),
        &"boot/native-mask".into(),
        1,
        "surface/native-mask",
        None,
    )
    .unwrap();
    let plan = &stage.planned_mask.plan;
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("born/native-mask"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("wake/native-mask")).unwrap();
    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(plan.source_document_id.clone()),
            checked_plot_id: Some(plan.checked_plot_id.clone()),
            expanded_plot_id: Some(plan.expanded_plot_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: alloc::vec![SignId::from("face/native-mask")],
        },
        alloc::vec![PresentationSubject {
            identity: "front/native-mask".into(),
            role: PresentationRole::Document,
            name: "Native Mask".into()
        }],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let presentation = Presentation::new_with_semantics(
        presentation.revision,
        presentation.basis,
        presentation.subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
        alloc::vec![conduit_presentation::PresentationAction {
            identity: "action/activate".into(),
            intent: "action/activate".into(),
            target: "front/native-mask".into(),
            name: "Activate".into(),
            arguments: Vec::new(),
            disclosure: conduit_presentation::PresentationDisclosureLevel::CurrentAction,
            availability: conduit_presentation::PresentationActionAvailability::Available,
        }],
        presentation.disclosures,
    )
    .unwrap();
    let pending = PreparedNativeMaskPlay::prepare(
        &stage.planned_mask,
        &presentation,
        sequence,
        "front/native-mask",
        "surface/native-mask",
        HostBaseId::from("display/native-mask"),
    )
    .unwrap();
    let request = pending.renderer_request();
    let show = &request.prepared_show().show;
    let mut compositor = NativeCompositor::admitted(
        CompositorAdmission::new(
            show.host_id.clone(),
            show.boot_id.clone(),
            show.offer_generation,
            show.presenter_implementation_id.clone(),
            request.display_base_id().clone(),
            alloc::vec![show.placement_id.clone()],
            alloc::vec![show.target_subject.clone()],
        )
        .unwrap(),
    );
    compositor
        .admit_surface(
            &show.target_subject,
            LayoutRect {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            0,
        )
        .unwrap();
    (pending, compositor)
}

fn stage(
    pending: &PreparedNativeMaskPlay,
    compositor: &mut NativeCompositor,
) -> crate::native_compositor::CompositionReceipt {
    let request = pending.renderer_request();
    compositor
        .update_pending_mask_surface(
            request.presentation(),
            request.prepared_show(),
            request.display_base_id(),
            &GraphicsScene::empty(),
        )
        .unwrap()
        .clone()
}

#[test]
fn renderer_waits_for_exact_scanout_before_delivering_its_show_fore() {
    let (pending, mut compositor) = fixture_scanout(1);
    let request = pending.renderer_request();
    assert_eq!(
        run(
            &request.prepared_show().planned_mask,
            request.presentation(),
            1
        ),
        Err(NativeMaskPlayError::PendingRenderer)
    );
    let staged = stage(&pending, &mut compositor);
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
    assert!(compositor.focus_surface("surface/native-mask").is_err());
    let mut display = FixturePixelTarget {
        writes: 0,
        fail: false,
    };
    let frame = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(display.writes, 256);
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    assert_eq!(ack.frame_sequence(), frame.frame_sequence);
    let receipt = pending.complete(&ack).unwrap();
    assert_eq!(receipt.mask_plan_id, staged.plan_id);
    assert_eq!(receipt.active_play_id, staged.active_play_id);
    assert_eq!(receipt.presentation_id, staged.presentation_id.as_str());
    assert!(receipt.kernel_signs > 0);
    compositor.focus_surface("surface/native-mask").unwrap();
}

#[test]
fn other_play_scanout_cannot_complete_pending_renderer() {
    let (other, mut compositor) = fixture_scanout(2);
    let staged = stage(&other, &mut compositor);
    compositor
        .compose_frame(&mut FixturePixelTarget {
            writes: 0,
            fail: false,
        })
        .unwrap();
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    let (pending, _) = fixture_scanout(1);
    assert_eq!(
        pending.complete(&ack),
        Err(NativeMaskPlayError::RendererMismatch)
    );
    other.cancel().unwrap();
}

#[test]
fn display_failure_has_no_ack_and_failed_or_cancelled_execution_returns_no_show() {
    let (pending, mut compositor) = fixture_scanout(1);
    let staged = stage(&pending, &mut compositor);
    assert!(
        compositor
            .compose_frame(&mut FixturePixelTarget {
                writes: 0,
                fail: true
            })
            .is_err()
    );
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
    assert!(compositor.focus_surface("surface/native-mask").is_err());
    assert_eq!(pending.fail(), NativeMaskPlayError::RendererFailed);
    let (pending, _) = fixture_scanout(1);
    pending.cancel().unwrap();
}

fn fixture_session(sequence: u64) -> NativeMaskInteractionSession {
    let (pending, mut compositor) = fixture_scanout(sequence);
    let staged = stage(&pending, &mut compositor);
    compositor
        .compose_frame(&mut FixturePixelTarget {
            writes: 0,
            fail: false,
        })
        .unwrap();
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    pending.complete_render(&ack).unwrap()
}

fn fixture_interaction(
    session: &NativeMaskInteractionSession,
) -> conduit_presentation::FaceInteraction {
    conduit_presentation::FaceInteraction::new(
        &session.play.renderer.presentation,
        session.show(),
        "action/activate",
        "front/native-mask",
        Vec::new(),
        1,
    )
    .unwrap()
}

#[test]
fn exact_interaction_crosses_same_mask_host_call_and_fore() {
    let session = fixture_session(1);
    let show = session.show().clone();
    let receipt = session.render_receipt();
    assert_eq!(
        show.show.lifecycle,
        conduit_presentation::ManifestationLifecycle::Available
    );
    assert_eq!(receipt.active_play_id, show.show.active_play_id);
    assert_eq!(session.request.request, conduit_kernel::RequestId(1));
    assert!(
        session
            .play
            .scheduler
            .remote_egress_terminal_disposition(
                session.play.interaction_fore.endpoint,
                session.play.interaction_fore.cord,
            )
            .unwrap()
            .is_none()
    );
    let interaction = fixture_interaction(&session);
    let correlation = session.interact(interaction.clone()).unwrap();
    assert_eq!(correlation.interaction, interaction);
    assert_eq!(correlation.show_id, show.show_id);
}

#[test]
fn stale_face_show_and_substituted_action_are_refused() {
    use conduit_presentation::FaceInteractionRefusal as Refusal;
    for (field, refusal) in [
        (0, Refusal::StaleFace),
        (1, Refusal::StaleShow),
        (2, Refusal::UnknownAction),
        (3, Refusal::WrongTarget),
        (4, Refusal::MalformedEncoding),
    ] {
        let session = fixture_session(1);
        let mut proposed = fixture_interaction(&session);
        match field {
            0 => proposed.face_revision += 1,
            1 => proposed.show_id = "show/stale".into(),
            2 => proposed.action_id = "action/substituted".into(),
            3 => proposed.target = "front/substituted".into(),
            _ => proposed.sequence += 1,
        }
        assert_eq!(
            session.interact(proposed),
            Err(NativeMaskPlayError::Interaction(refusal))
        );
    }
    let session = fixture_session(1);
    let other = fixture_session(2);
    let interaction = fixture_interaction(&other);
    assert_eq!(
        session.interact(interaction),
        Err(NativeMaskPlayError::Interaction(Refusal::StaleShow))
    );
    other.cancel().unwrap();
}

#[test]
fn no_input_normal_close_and_cancel_are_distinct_kernel_outcomes() {
    use conduit_kernel::scheduler::SchedulerStatus;
    let session = fixture_session(1);
    let rendered = session.render_receipt().clone();
    let closed = session.close_without_input().unwrap();
    assert_eq!(closed.active_play_id, rendered.active_play_id);
    assert!(closed.kernel_signs > rendered.kernel_signs);
    let mut session = fixture_session(2);
    session.play.scheduler.cancel().unwrap();
    assert_eq!(
        session.play.scheduler.step().unwrap(),
        SchedulerStatus::Cancelled
    );
    assert_eq!(
        session.interact(fixture_interaction(&fixture_session(2))),
        Err(NativeMaskPlayError::Cancelled)
    );
    fixture_session(3).cancel().unwrap();
}

#[test]
fn finite_value_store_pressure_returns_no_interaction_correlation() {
    let mut session = fixture_session(1);
    let interaction = fixture_interaction(&session);
    let mut stored = 0;
    while session.play.scheduler.store_host_value(&[1]).is_ok() {
        stored += 1;
    }
    assert!(stored > 0 && stored <= VALUES);
    assert_eq!(
        session.interact(interaction),
        Err(NativeMaskPlayError::Pressure)
    );
}

#[test]
fn storage_preparation_checks_the_exact_plan_queue_and_host_result_budgets() {
    let (pending, _) = fixture_scanout(1);
    let fragment = &pending
        .renderer_request()
        .prepared_show()
        .planned_mask
        .plan
        .fragments[0];
    let mut lowered = lower_plan_fragment(fragment).unwrap();
    assert_eq!(lowered.cord_value_slots, CORDS as u16);
    assert_eq!(
        lowered.cord_value_bytes,
        5 * MAX_MASK_VALUE_BYTES as u32 + INTERACTION_VALUE_BYTES as u32
    );
    assert_eq!(admitted_value_bytes(&lowered), Ok(VALUE_BYTES as u32));
    lowered.cord_value_bytes += 1;
    assert_eq!(
        admitted_value_bytes(&lowered),
        Err(NativeMaskPlayError::Pressure)
    );
    lowered.cord_value_bytes -= 1;
    lowered.cord_value_slots += 1;
    assert_eq!(
        admitted_value_bytes(&lowered),
        Err(NativeMaskPlayError::Shape)
    );
    lowered.cord_value_slots -= 1;
    lowered.host_calls[0].maximum_in_flight = 2;
    assert_eq!(
        admitted_value_bytes(&lowered),
        Err(NativeMaskPlayError::Shape)
    );
}

#[test]
fn complete_large_faces_cross_the_fore_and_oversized_faces_are_refused() {
    let (fixture, _) = fixture_scanout(1);
    let request = fixture.renderer_request();
    let original = request.presentation();
    let make_face = |paragraphs: usize| {
        let mut subjects = original.subjects.clone();
        let mut text = original.text.clone();
        for index in 0..paragraphs {
            let subject = alloc::format!("document/paragraph/{index}");
            subjects.push(PresentationSubject {
                identity: subject.clone(),
                role: PresentationRole::Document,
                name: alloc::format!("Paragraph {index}"),
            });
            text.push(conduit_presentation::PresentationText {
                subject,
                text: "x".repeat(1024),
            });
        }
        Presentation::new_with_semantics(
            original.revision,
            original.basis.clone(),
            subjects,
            original.relationships.clone(),
            original.properties.clone(),
            text,
            original.actions.clone(),
            original.disclosures.clone(),
        )
        .unwrap()
    };
    let prepare = |face: &Presentation| {
        PreparedNativeMaskPlay::prepare(
            &request.prepared_show().planned_mask,
            face,
            2,
            "front/native-mask",
            "surface/native-mask",
            request.display_base_id().clone(),
        )
    };
    let face = make_face(10);
    let bytes = serde_json::to_vec(&face).unwrap();
    assert!(bytes.len() >= 11_550 && bytes.len() <= MAX_MASK_VALUE_BYTES);
    let pending = prepare(&face).unwrap();
    assert_eq!(pending.renderer_request().presentation(), &face);
    pending.cancel().unwrap();
    let oversized = make_face(20);
    assert!(serde_json::to_vec(&oversized).unwrap().len() > MAX_MASK_VALUE_BYTES);
    assert!(matches!(
        prepare(&oversized),
        Err(NativeMaskPlayError::Value)
    ));
}

#[test]
fn prepared_storage_never_grows_and_runs_on_the_native_preparation_stack_bound() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        use conduit_kernel::ValueStorage;
        let session = fixture_session(1);
        let values = session.play.scheduler.values();
        assert_eq!(values.allocation_capacities(), (VALUES, VALUES * STORAGE_VALUE_BYTES));
        assert_eq!(values.byte_capacity(), VALUE_BYTES as u32);
        let before = values.allocation_capacities();
        let interaction = fixture_interaction(&session);
        // Exercise the store at its largest admitted single-value bound.
        let mut session = session;
        let value = session.play.scheduler.store_host_value(&[0; STORAGE_VALUE_BYTES]).unwrap();
        assert_eq!(session.play.scheduler.values().allocation_capacities(), before);
        session.play.scheduler.discard_host_value(value).unwrap();
        session.interact(interaction).unwrap();
        assert!(core::mem::size_of::<Scheduler>() < 32 * 1024);
        assert!(core::mem::size_of::<PreparedNativeMaskPlay>() < 4 * 1024);
        std::println!("native Mask envelopes: scheduler={} bytes, prepared={} bytes, reserved value payload={} bytes, logical value budget={} bytes; 1 MiB preparation stack test passed",
            core::mem::size_of::<Scheduler>(), core::mem::size_of::<PreparedNativeMaskPlay>(),
            VALUES * STORAGE_VALUE_BYTES, VALUE_BYTES);
    }).unwrap().join().unwrap();
}

#[test]
fn interaction_fore_waits_for_delivery_without_changing_or_duplicating_the_value() {
    use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};
    let mut session = fixture_session(1);
    let bytes = fixture_interaction(&session).encode();
    let value = session.play.scheduler.store_host_value(&bytes).unwrap();
    session
        .play
        .scheduler
        .complete_host_call(
            session.request.node,
            session.request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value, bytes.len() as u32).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let port = session.play.interaction_fore.clone();
    for _ in 0..64 {
        if session
            .play
            .scheduler
            .remote_egress_offer(port.endpoint, port.cord)
            .unwrap()
            .is_some()
        {
            break;
        }
        session.play.scheduler.step().unwrap();
    }
    let offered = session
        .play
        .scheduler
        .remote_egress_offer(port.endpoint, port.cord)
        .unwrap()
        .unwrap();
    for _ in 0..4 {
        session.play.scheduler.step().unwrap();
        let retained = session
            .play
            .scheduler
            .remote_egress_offer(port.endpoint, port.cord)
            .unwrap()
            .unwrap();
        assert_eq!(retained.sequence, offered.sequence);
        assert_eq!(retained.value, offered.value);
        assert_eq!(
            session.play.scheduler.host_value(retained.value).unwrap(),
            bytes
        );
        assert!(
            session
                .play
                .scheduler
                .remote_egress_terminal_disposition(port.endpoint, port.cord)
                .unwrap()
                .is_none()
        );
    }
    session.cancel().unwrap();
}
