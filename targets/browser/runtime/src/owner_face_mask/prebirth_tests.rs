//! Zero-Body checked Fore exchange; simulated effects only, no Birth is applied.
use super::*;

#[test]
fn checked_browser_mask_fore_carries_prebirth_face_and_show_without_a_body() {
    let face: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/prebirth-face.json"
    ))
    .unwrap();
    assert!(face.basis.body_id.is_none());
    let host = crate::installed_browser::membership_advertisement(
        HostId::from("host/prebirth-conformance"),
        BootId::from("boot/prebirth-conformance"),
    );
    let planned = crate::workspace_mask::plan::planned_mask(
        &host,
        crate::workspace_mask::plan::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let play = bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    let show = MaskShow::prepared(
        &planned,
        &face,
        play,
        face.subjects[0].identity.clone(),
        "browser/prebirth-fixture-document".into(),
        SignId::from("sign/prebirth-mask-prepared"),
    )
    .unwrap();
    let fragment = &planned.plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).unwrap();
    let input =
        execution::exact_boundary(&lowered.fore_ports, "face", PortDirection::Input).unwrap();
    let output =
        execution::exact_boundary(&lowered.fore_ports, "show", PortDirection::Output).unwrap();
    let mut scheduler = execution::mask_scheduler(fragment, &lowered).unwrap();
    let bytes = serde_json::to_vec(&face).unwrap();
    assert_eq!(
        scheduler
            .admit_remote_input(input.endpoint, input.cord, 0, &bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    );
    scheduler
        .close_remote_input(input.endpoint, input.cord)
        .unwrap();
    let mut pending = None;
    for _ in 0..64 {
        if let Some(request) = scheduler.next_host_request() {
            pending = Some(request);
            break;
        }
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    let pending = pending.expect("finite Mask must request its DOM Host Call");
    assert_eq!(scheduler.host_value(pending.input.value).unwrap(), bytes);
    // Simulated DOM effect acknowledgement, not Chromium or physical output.
    let show_bytes = serde_json::to_vec(&show.show).unwrap();
    let stored = scheduler.store_host_value(&show_bytes).unwrap();
    scheduler
        .complete_host_call(
            pending.node,
            pending.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(stored, MASK_BYTES).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let mut emitted = None;
    for _ in 0..64 {
        if let Some(offer) = scheduler
            .remote_egress_offer(output.endpoint, output.cord)
            .unwrap()
        {
            emitted = Some(offer);
            break;
        }
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    let emitted = emitted.expect("finite Mask must emit the supplied Show");
    assert_eq!(scheduler.host_value(emitted.value).unwrap(), show_bytes);
    scheduler
        .remote_egress_accept(output.endpoint, output.cord, emitted.sequence)
        .unwrap();
    scheduler
        .remote_egress_delivered(output.endpoint, output.cord, emitted.sequence)
        .unwrap();
    assert!(FaceInteraction::new(
        &face,
        &show,
        "creche.suggest",
        "birth/encounter/prebirth-conformance/navigation",
        vec![],
        1
    )
    .is_err());
    let available = show
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("sign/prebirth-fixture-acknowledged"),
        )
        .unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "creche.suggest")
        .unwrap();
    assert!(FaceInteraction::new(
        &face,
        &available,
        "invented/prebirth",
        &action.target,
        vec![],
        1
    )
    .is_err());
    assert!(FaceInteraction::new(
        &face,
        &available,
        &action.identity,
        "invented/target",
        vec![],
        1
    )
    .is_err());
    let mut stale = face.clone();
    stale.revision += 1;
    assert!(FaceInteraction::new(
        &stale,
        &available,
        &action.identity,
        &action.target,
        vec![],
        1
    )
    .is_err());
    assert!(FaceInteraction::new(
        &face,
        &available,
        "creche.plot.1",
        "birth/encounter/prebirth-conformance/initial-plots/1",
        vec![],
        1
    )
    .is_err());
    let name = face
        .actions
        .iter()
        .find(|action| action.identity == "creche.name")
        .unwrap();
    assert!(FaceInteraction::new(
        &face,
        &available,
        &name.identity,
        &name.target,
        vec![FaceInteractionArgument {
            name: "value".into(),
            value_kind: "value/text".into(),
            value: vec![b'x'; 65]
        }],
        1
    )
    .is_err());
    for lifecycle in [
        ManifestationLifecycle::Closed,
        ManifestationLifecycle::Replaced,
        ManifestationLifecycle::Failed,
    ] {
        let retired = if lifecycle == ManifestationLifecycle::Failed {
            available
                .fail(
                    conduit_presentation::ManifestationFailure::AdapterUnavailable,
                    SignId::from("sign/prebirth-fixture-lost"),
                )
                .unwrap()
        } else {
            available
                .transition(lifecycle, SignId::from("sign/prebirth-fixture-retired"))
                .unwrap()
        };
        assert!(
            FaceInteraction::new(&face, &retired, &action.identity, &action.target, vec![], 1)
                .is_err()
        );
    }
    let interaction = FaceInteraction::new(
        &face,
        &available,
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    let mut request = None;
    for _ in 0..64 {
        if let Some(pending) = scheduler.next_host_request() {
            request = Some(pending);
            break;
        }
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    let request = request.expect("finite prebirth Mask must request typed interaction");
    let encoded = interaction.encode();
    let value = scheduler.store_host_value(&encoded).unwrap();
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value, MASK_BYTES).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let boundary =
        execution::exact_boundary(&lowered.fore_ports, "interaction", PortDirection::Output)
            .unwrap();
    let mut returned = None;
    for _ in 0..64 {
        if let Some(offer) = scheduler
            .remote_egress_offer(boundary.endpoint, boundary.cord)
            .unwrap()
        {
            returned = Some(offer);
            break;
        }
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    let returned = returned.expect("finite prebirth Mask must emit typed return");
    let returned_bytes = scheduler.host_value(returned.value).unwrap();
    assert_eq!(returned_bytes, encoded);
    assert_eq!(
        FaceInteraction::decode(returned_bytes).unwrap(),
        interaction
    );
    assert_eq!(serde_json::to_vec(&face).unwrap(), bytes);
    assert!(face.basis.body_id.is_none());
    assert!(face.basis.wake_id.is_none());
    assert!(face.basis.plan_id.is_none());
    assert!(face.basis.active_play_id.is_none());
}
