//! Source map and actual software terminal effect; no installed input dispatch proof.
use super::*;
use conduit_body::{Body, BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
use conduit_core::{resource_offer, PlannedActivationEntry, INPUT_RESOURCE_CLASS};
use conduit_kernel::NodeId;
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
    lowering::{KernelIdentityMap, LoweredForePort, FIXED_KERNEL_STORAGE_PROFILE},
};
use conduit_presentation::{ManifestationLifecycle, ParsecMapCommand};

fn terminal_host() -> crate::StdHost {
    let mut advertisement = crate::composition::build_advertisement(
        crate::StdHostConfig {
            host_id: "proof/parsec-terminal".into(),
            boot_id: "proof/parsec-terminal/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        crate::StdHostComposition::reference(),
        None,
        None,
        None,
        false,
    );
    let mut offers = crate::flow_activation::required_parsec_source_offers().unwrap();
    offers.extend([
        crate::flow_activation::parsec_combine_offer().unwrap(),
        crate::flow_activation::parsec_scan_offer(32).unwrap(),
        crate::flow_activation::required_parsec_input_policy_offer().unwrap(),
        conduit_std_offers::hosted_keyboard_offer(
            "proof/parsec-terminal-keys",
            "proof/parsec-terminal-keys@1",
        ),
        conduit_std_offers::button::offer(),
    ]);
    offers.extend(crate::terminal_mask_execution::planning::attached_terminal_capabilities());
    for offer in offers {
        if let Some(existing) = advertisement
            .capabilities
            .iter()
            .find(|existing| existing.capability_id == offer.capability_id)
        {
            assert_eq!(existing, &offer);
        } else {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement.resources.push(resource_offer(
        "proof/parsec-terminal-input",
        INPUT_RESOURCE_CLASS,
        2,
    ));
    advertisement
        .resources
        .extend(crate::terminal_mask_execution::planning::attached_terminal_resources());
    advertisement.resources.sort();
    // Every fixture capability/resource is selected before the actual runner is constructed.
    crate::StdHost::from_advertisement(advertisement).unwrap()
}

pub(super) fn fixture() -> (
    crate::StdHost,
    BodyPlan,
    BodyPlayIdentity,
    KernelIdentityMap,
    LoweredForePort,
    NodeId,
) {
    let host = terminal_host();
    let plan = crate::BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "sign/map-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/map-wake".into()).unwrap().1;
    let body_plan = BodyPlan::seal(
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
    let prospective = BodyPlayIdentity::bind(&body_plan, 0);
    let lowered = lower_local_fragment_set_from_plans(
        &[&body_plan.plots[0].plan],
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 1,
            nodes: super::super::super::MAX_NODES as u16,
            cords: super::super::super::MAX_CORDS as u16,
            queue_slots: super::super::super::MAX_QUEUE_SLOTS as u16,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 1024 * 1024,
        },
    )
    .unwrap();
    let partition = &lowered.partitions[0];
    let port = partition
        .fore_ports
        .iter()
        .find(|p| p.front_port_id.as_str() == "states")
        .unwrap()
        .clone();
    let PlannedActivationEntry::Scan(scan) = &body_plan.plots[0].plan.activations[0] else {
        panic!("scan");
    };
    let node = partition
        .identity
        .node_for_placement(&scan.owner_placement_id)
        .unwrap();
    (
        host,
        body_plan,
        prospective,
        partition.identity.clone(),
        port,
        node,
    )
}

#[test]
fn actual_terminal_effect_owns_source_action_face_and_available_show() {
    let (host, plan, play, identity, _, _) = fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
    // Unacknowledged access retires the previous owner; prepare a fresh exact route.
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    owner
        .present(&map, host.advertisement(), &mut output)
        .unwrap();
    assert!(!output.is_empty());
    let (face, show) = owner.current_face_show(&map, host.advertisement()).unwrap();
    assert_eq!(face.actions.len(), 1);
    assert_eq!(face.actions[0].identity, "game/start");
    assert_eq!(
        face.basis.active_play_id.as_ref(),
        Some(&play.active_play_id)
    );
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    show.validate(face).unwrap();
    owner.retire();
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
}

#[test]
fn acknowledged_source_phase_change_invalidates_old_show_and_projects_new_action() {
    let (host, plan, play, identity, port, node) = fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    owner
        .present(&map, host.advertisement(), &mut Vec::new())
        .unwrap();
    let old_show = owner
        .current_face_show(&map, host.advertisement())
        .unwrap()
        .1
        .clone();
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host.advertisement()).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = ParsecMapCommand::new("game/start".into(), 1, false)
        .unwrap()
        .encode()
        .unwrap();
    let result = evaluator
        .evaluate(map.current_state().unwrap(), &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 0, &result).unwrap();
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
    let mut current = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    current
        .present(&map, host.advertisement(), &mut Vec::new())
        .unwrap();
    let (face, show) = current
        .current_face_show(&map, host.advertisement())
        .unwrap();
    assert_eq!(face.actions[0].identity, "game/shoot");
    assert_ne!(show.show_id, old_show.show_id);
    assert!(old_show.validate(face).is_err());
}

#[test]
fn failed_terminal_effect_changed_host_and_unissued_map_never_grant_show() {
    struct Fails;
    impl Write for Fails {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    struct FailsFlush {
        written: usize,
    }
    impl Write for FailsFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.written += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }
    let (host, plan, play, identity, _, _) = fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    assert!(owner
        .present(&map, host.advertisement(), &mut Vec::new())
        .is_err());
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    assert!(owner
        .present(&map, host.advertisement(), &mut Fails)
        .is_err());
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut flush = FailsFlush { written: 0 };
    assert!(owner
        .present(&map, host.advertisement(), &mut flush)
        .is_err());
    assert!(flush.written > 0);
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    owner
        .present(&map, host.advertisement(), &mut Vec::new())
        .unwrap();
    let mut changed = host.advertisement().clone();
    changed.offer_generation.0 += 1;
    assert!(owner.current_face_show(&map, &changed).is_err());
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
}

#[test]
fn unchanged_map_keeps_show_and_same_play_phase_update_advances_mask_sequence() {
    let (host, plan, play, identity, port, node) = fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    let first = owner
        .current_face_show(&map, host.advertisement())
        .unwrap()
        .1
        .clone();
    let written = output.len();
    assert!(!owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    assert_eq!(output.len(), written);
    assert_eq!(
        owner
            .current_face_show(&map, host.advertisement())
            .unwrap()
            .1,
        &first
    );
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host.advertisement()).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = ParsecMapCommand::new("game/start".into(), 1, false)
        .unwrap()
        .encode()
        .unwrap();
    let result = evaluator
        .evaluate(map.current_state().unwrap(), &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 0, &result).unwrap();
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    let (face, second) = owner.current_face_show(&map, host.advertisement()).unwrap();
    assert_eq!(face.actions[0].identity, "game/shoot");
    assert_eq!(second.show.play_sequence, first.show.play_sequence + 1);
    assert_ne!(second.show_id, first.show_id);
    assert!(first.validate(face).is_err());
    assert!(!owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());

    let different_play = BodyPlayIdentity::bind(&plan, 1);
    let mut different_map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &different_play, &identity)
            .unwrap();
    different_map
        .confirm(host.advertisement(), &different_play)
        .unwrap();
    assert!(owner
        .ensure_present(&different_map, host.advertisement(), &mut output)
        .is_err());
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .is_err());
}

#[test]
fn fresh_owners_for_distinct_confirmed_body_plays_have_disjoint_mask_play_scopes() {
    let (host, plan, first_play, identity, _, _) = fixture();
    let mut first_map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &first_play, &identity)
            .unwrap();
    first_map
        .confirm(host.advertisement(), &first_play)
        .unwrap();
    let second_play = BodyPlayIdentity::bind(&plan, first_play.play_sequence + 1);
    let mut second_map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &second_play, &identity)
            .unwrap();
    second_map
        .confirm(host.advertisement(), &second_play)
        .unwrap();
    let mut first = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut second = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    first
        .ensure_present(&first_map, host.advertisement(), &mut Vec::new())
        .unwrap();
    second
        .ensure_present(&second_map, host.advertisement(), &mut Vec::new())
        .unwrap();
    let (first_face, first_show) = first
        .current_face_show(&first_map, host.advertisement())
        .unwrap();
    let (second_face, second_show) = second
        .current_face_show(&second_map, host.advertisement())
        .unwrap();
    assert_eq!(
        first_show.planned_mask.plan.plan_id,
        second_show.planned_mask.plan.plan_id
    );
    assert_eq!(
        first_face.actions[0].identity,
        second_face.actions[0].identity
    );
    assert_eq!(first_show.show.play_sequence, first_play.play_sequence * 64);
    assert_eq!(
        second_show.show.play_sequence,
        second_play.play_sequence * 64
    );
    assert_ne!(
        first_show.show.active_play_id,
        second_show.show.active_play_id
    );
    assert_ne!(first_show.show_id, second_show.show_id);
}

#[test]
fn body_scope_overflow_refuses_before_terminal_write() {
    let (host, plan, _, identity, _, _) = fixture();
    let play = BodyPlayIdentity::bind(&plan, u64::MAX);
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .is_err());
    assert!(output.is_empty());
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
}

#[test]
fn selected_show_count_refuses_before_write_and_unchanged_ensure_uses_no_slot() {
    let (host, plan, play, identity, _, _) = fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    map.confirm(host.advertisement(), &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host.advertisement(), 80, 24).unwrap();
    let mut output = Vec::new();
    assert!(owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    for _ in 0..64 {
        assert!(!owner
            .ensure_present(&map, host.advertisement(), &mut output)
            .unwrap());
    }
    assert_eq!(owner.rendered_shows, 1);
    for _ in 1..owner.maximum_shows {
        owner
            .present(&map, host.advertisement(), &mut output)
            .unwrap();
    }
    let written = output.len();
    let last = owner
        .current_face_show(&map, host.advertisement())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        last.show.play_sequence,
        play.play_sequence * 64 + u64::from(owner.maximum_shows - 1)
    );
    assert!(!owner
        .ensure_present(&map, host.advertisement(), &mut output)
        .unwrap());
    assert!(owner
        .present(&map, host.advertisement(), &mut output)
        .is_err());
    assert_eq!(output.len(), written);
    assert!(owner.current_face_show(&map, host.advertisement()).is_err());
}
