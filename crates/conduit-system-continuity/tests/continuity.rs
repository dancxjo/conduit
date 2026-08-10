use conduit_core::{
    bind_active_play, bind_sign, seal_plan, AuthorityGrantId, BootId, CapabilityId, FormIdentity,
    KindContractRevision, Observation, ObservationKind, Plan, PlanId, SignId, TerminalDisposition,
};
use conduit_observatory::{
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport, LineReport,
    ObservatorySnapshot, OfferFreshness, OperationalState, PlanLifecycle, PlayReport,
    RetentionReport, SNAPSHOT_SCHEMA,
};
use conduit_signal::triple;
use conduit_system_continuity::{
    ContinuityError, DelegatedTransitionGrant, DurableSystemId, HostInstance, RoleId,
    RoleRequirement, SystemRecord, TransitionCause, TransitionId,
};

fn available_host(advertisement: conduit_core::HostAdvertisement) -> HostReport {
    HostReport {
        capabilities: advertisement
            .capabilities
            .iter()
            .map(|offer| CapabilityStatusReport {
                capability_id: offer.capability_id.clone(),
                freshness: OfferFreshness::Fresh,
                support: CapabilitySupport::Supported,
                availability: CapabilityAvailability::Available,
            })
            .collect(),
        advertisement,
        state: OperationalState::Available,
    }
}

fn plays(plan: &Plan, play_sequence: u64) -> Vec<PlayReport> {
    plan.fragments
        .iter()
        .map(|fragment| PlayReport {
            active_play_id: bind_active_play(
                &plan.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                play_sequence,
            )
            .active_play_id,
            plan_id: plan.plan_id.clone(),
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            lifecycle: PlanLifecycle::Completed,
            terminal_disposition: Some(conduit_core::TerminalDisposition::Completed),
            failure_message: None,
            placements: Vec::new(),
            connections: Vec::new(),
        })
        .collect()
}

fn snapshot(
    plan: Plan,
    hosts: Vec<HostReport>,
    lines: Vec<conduit_core::LineOffer>,
    play_sequence: u64,
) -> ObservatorySnapshot {
    let plays = plays(&plan, play_sequence);
    let observations = plays
        .iter()
        .enumerate()
        .map(|(sequence, play)| {
            let identity = bind_sign(
                &play.host_id,
                &play.boot_id,
                Some(&play.active_play_id),
                sequence as u64,
            );
            Observation {
                sign_id: identity.sign_id,
                active_play_id: Some(play.active_play_id.clone()),
                presentation_id: None,
                host_id: play.host_id.clone(),
                boot_id: play.boot_id.clone(),
                plan_id: Some(plan.plan_id.clone()),
                placement_id: None,
                connection_id: None,
                kind: ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed,
                },
            }
        })
        .collect::<Vec<_>>();
    let retained_items = observations.len() as u32;
    ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.to_owned(),
        hosts,
        bases: Vec::new(),
        lines: lines
            .into_iter()
            .map(|offer| LineReport {
                offer,
                state: OperationalState::Available,
            })
            .collect(),
        plays,
        plans: vec![plan],
        observations,
        historical_observations: Vec::new(),
        sealed_boot_provenance: Vec::new(),
        retention: RetentionReport {
            item_capacity: 32,
            retained_items,
            dropped_items: 0,
        },
    }
}

fn host(advertisement: &conduit_core::HostAdvertisement) -> HostInstance {
    HostInstance {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
    }
}

fn requirements(
    plan: &Plan,
    advertisements: &[&conduit_core::HostAdvertisement],
) -> Vec<RoleRequirement> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| {
            let offer = advertisements
                .iter()
                .find(|advertisement| {
                    advertisement.host_id == placement.host_id
                        && advertisement.boot_id == placement.boot_id
                })
                .unwrap()
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .unwrap();
            RoleRequirement {
                role_id: RoleId::from(placement.gear_id.as_str()),
                gear_id: placement.gear_id.clone(),
                checked_face: offer.checked_face(),
            }
        })
        .collect()
}

struct Fixture {
    exact: triple::ExactTripleSignalPlan,
    requirements: Vec<RoleRequirement>,
    members: Vec<HostInstance>,
    grant: DelegatedTransitionGrant,
    snapshot: ObservatorySnapshot,
}

fn fixture() -> Fixture {
    let exact = triple::exact_plan().unwrap();
    let requirements = requirements(
        &exact.plan,
        &[
            &exact.source_advertisement,
            &exact.browser_advertisement,
            &exact.pico_advertisement,
        ],
    );
    let members = vec![
        host(&exact.source_advertisement),
        host(&exact.browser_advertisement),
        host(&exact.pico_advertisement),
    ];
    let grant = DelegatedTransitionGrant {
        grant_id: AuthorityGrantId::from("grant/std-may-reboot-pico-once"),
        controller: members[0].clone(),
        subject: members[2].clone(),
        capability_id: CapabilityId::from("capability/pico-reboot"),
        selected_line_id: exact.pico_line.line_id.clone(),
        maximum_transitions: 1,
        proof_window_ticks: 2,
        sign_sequence_base: 40,
    };
    let snapshot = snapshot(
        exact.plan.clone(),
        vec![
            available_host(exact.source_advertisement.clone()),
            available_host(exact.browser_advertisement.clone()),
            available_host(exact.pico_advertisement.clone()),
        ],
        vec![exact.browser_line.clone(), exact.pico_line.clone()],
        1,
    );
    Fixture {
        exact,
        requirements,
        members,
        grant,
        snapshot,
    }
}

fn record(fixture: &Fixture) -> SystemRecord {
    SystemRecord::from_snapshot(
        DurableSystemId::from("system/triple-signal"),
        fixture.exact.plan.checked_form_id.clone(),
        fixture.members.clone(),
        fixture.requirements.clone(),
        &fixture.exact.plan.plan_id,
        vec![fixture.grant.clone()],
        &fixture.snapshot,
    )
    .unwrap()
}

fn replacement(fixture: &Fixture) -> (conduit_core::HostAdvertisement, Plan, ObservatorySnapshot) {
    let mut pico = fixture.exact.pico_advertisement.clone();
    let new_boot = BootId::from("s6/replacement-pico-boot");
    pico.boot_id = new_boot.clone();
    pico.offer_generation = conduit_core::OfferGeneration(2);

    let old_boot = &fixture.exact.pico_advertisement.boot_id;
    let mut fragments = fixture.exact.plan.fragments.clone();
    for fragment in &mut fragments {
        if &fragment.boot_id == old_boot {
            fragment.boot_id = new_boot.clone();
            fragment.offer_generation = conduit_core::OfferGeneration(2);
        }
        for placement in &mut fragment.placements {
            if &placement.boot_id == old_boot {
                placement.boot_id = new_boot.clone();
                placement.offer_generation = conduit_core::OfferGeneration(2);
            }
        }
        for connection in &mut fragment.connections {
            if let Some(link) = &mut connection.selected_line {
                if &link.binding.source.boot_id == old_boot {
                    link.binding.source.boot_id = new_boot.clone();
                }
                if &link.binding.sink.boot_id == old_boot {
                    link.binding.sink.boot_id = new_boot.clone();
                }
            }
            for candidate in &mut connection.admitted_lines {
                if &candidate.binding.source.boot_id == old_boot {
                    candidate.binding.source.boot_id = new_boot.clone();
                }
                if &candidate.binding.sink.boot_id == old_boot {
                    candidate.binding.sink.boot_id = new_boot.clone();
                }
            }
        }
    }
    let plan = seal_plan(
        FormIdentity {
            source_document_id: fixture.exact.plan.source_document_id.clone(),
            checked_form_id: fixture.exact.plan.checked_form_id.clone(),
            expanded_form_id: fixture.exact.plan.expanded_form_id.clone(),
        },
        fragments,
    );
    assert_ne!(plan.plan_id, fixture.exact.plan.plan_id);

    let browser_line = fixture.exact.browser_line.clone();
    let mut pico_line = fixture.exact.pico_line.clone();
    if pico_line.binding.source.host_id == pico.host_id {
        pico_line.binding.source.boot_id = new_boot.clone();
    }
    if pico_line.binding.sink.host_id == pico.host_id {
        pico_line.binding.sink.boot_id = new_boot;
    }
    // The browser link is intentionally unchanged; link reachability is not membership.
    let replacement_snapshot = snapshot(
        plan.clone(),
        vec![
            available_host(fixture.exact.source_advertisement.clone()),
            available_host(fixture.exact.browser_advertisement.clone()),
            available_host(pico.clone()),
        ],
        vec![browser_line, pico_line],
        2,
    );
    (pico, plan, replacement_snapshot)
}

#[test]
fn exact_roles_bind_to_three_explicit_members_one_plan_and_distinct_plays() {
    let fixture = fixture();
    let record = record(&fixture);

    assert_eq!(record.members.len(), 3);
    assert_eq!(record.assignments.len(), 4);
    assert_eq!(record.observed_links.len(), 2);
    assert_eq!(record.play_ids.len(), 3);
    assert_eq!(record.sign_ids.len(), 3);
    assert_eq!(record.plan_id, fixture.exact.plan.plan_id);
    assert!(record.assignments.iter().any(|assignment| {
        assignment.role_id == RoleId::from("light")
            && assignment.host == host(&fixture.exact.pico_advertisement)
    }));
}

#[test]
fn reboot_acceptance_termination_replacement_and_replan_remain_distinct() {
    let fixture = fixture();
    let old = record(&fixture);
    let accepted = old
        .begin_transition(
            TransitionId::from("transition/pico-reboot-1"),
            fixture.grant.subject.clone(),
            TransitionCause::Delegated {
                grant_id: fixture.grant.grant_id.clone(),
                controller: fixture.grant.controller.clone(),
                accepted_sign_id: SignId::from("sign/reboot-accepted"),
            },
        )
        .unwrap();
    let terminated = accepted.old_boot_terminated(SignId::from("sign/old-boot-terminal"));
    assert_eq!(
        terminated
            .clone()
            .observe_replacement(available_host(fixture.exact.pico_advertisement.clone())),
        Err(ContinuityError::ReplacementBootReused)
    );

    let (pico, new_plan, new_snapshot) = replacement(&fixture);
    let assessment = terminated
        .observe_replacement(available_host(pico.clone()))
        .unwrap()
        .assess(&old);
    assert_eq!(assessment.stale_roles, vec![RoleId::from("light")]);
    assert_eq!(assessment.compatible_replacements.len(), 1);
    assert!(assessment.stale_authority.contains(&fixture.grant.grant_id));

    let mut members = fixture.members.clone();
    members[2] = host(&pico);
    let new_requirements = requirements(
        &new_plan,
        &[
            &fixture.exact.source_advertisement,
            &fixture.exact.browser_advertisement,
            &pico,
        ],
    );
    let replacement_record = SystemRecord::from_snapshot(
        DurableSystemId::from("system/triple-signal"),
        new_plan.checked_form_id.clone(),
        members,
        new_requirements,
        &new_plan.plan_id,
        Vec::new(),
        &new_snapshot,
    )
    .unwrap();
    let resumed = assessment.accept_replanned(replacement_record).unwrap();
    assert_eq!(resumed.plan_id, new_plan.plan_id);
    assert!(resumed.transition_grants.is_empty());
    assert!(resumed
        .play_ids
        .iter()
        .all(|play| !old.play_ids.contains(play)));
}

#[test]
fn compatible_face_does_not_inherit_assignment_grant_plan_or_play() {
    let fixture = fixture();
    let old = record(&fixture);
    let accepted = old
        .begin_transition(
            TransitionId::from("transition/local-pico-reboot"),
            fixture.grant.subject.clone(),
            TransitionCause::Local {
                accepted_sign_id: SignId::from("sign/local-request"),
            },
        )
        .unwrap();
    let (mut pico, _, _) = replacement(&fixture);
    let offer = &mut pico.capabilities[0];
    offer.capability_id = CapabilityId::from("replacement/equal-face-led");
    offer.kind_id = conduit_core::kind_id("replacement/show");
    offer.kind_contract_revision = KindContractRevision::from("replacement/show@9");
    offer.implementation.implementation_id =
        conduit_core::ImplementationId::from("replacement/led-v9");
    let assessment = accepted
        .old_boot_terminated(SignId::from("sign/local-old-terminal"))
        .observe_replacement(available_host(pico))
        .unwrap()
        .assess(&old);

    assert_eq!(assessment.compatible_replacements.len(), 1);
    let mut unchanged = old.clone();
    unchanged.plan_id = PlanId::from(assessment.prior_plan_id.as_str());
    assert_eq!(
        assessment.accept_replanned(unchanged),
        Err(ContinuityError::ReplanStillUsesOldPlan)
    );
}

#[test]
fn membership_availability_and_delegated_authority_fail_independently() {
    let fixture = fixture();
    let mut members = fixture.members.clone();
    members.pop();
    assert!(matches!(
        SystemRecord::from_snapshot(
            DurableSystemId::from("system/missing-member"),
            fixture.exact.plan.checked_form_id.clone(),
            members,
            fixture.requirements.clone(),
            &fixture.exact.plan.plan_id,
            Vec::new(),
            &fixture.snapshot,
        ),
        Err(ContinuityError::MissingMember(_))
    ));

    let mut unavailable = fixture.snapshot.clone();
    unavailable.hosts[2].capabilities[0].availability = CapabilityAvailability::Unavailable;
    assert!(matches!(
        SystemRecord::from_snapshot(
            DurableSystemId::from("system/unavailable"),
            fixture.exact.plan.checked_form_id.clone(),
            fixture.members.clone(),
            fixture.requirements.clone(),
            &fixture.exact.plan.plan_id,
            Vec::new(),
            &unavailable,
        ),
        Err(ContinuityError::CapabilityUnavailable(_))
    ));

    let record = record(&fixture);
    assert_eq!(
        record.begin_transition(
            TransitionId::from("transition/ambient-authority"),
            fixture.grant.subject,
            TransitionCause::Delegated {
                grant_id: AuthorityGrantId::from("grant/not-issued"),
                controller: fixture.grant.controller,
                accepted_sign_id: SignId::from("sign/not-authorized"),
            },
        ),
        Err(ContinuityError::MissingTransitionGrant)
    );
}
