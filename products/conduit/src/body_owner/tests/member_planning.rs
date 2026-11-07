//! Ordinary installed planning over authenticated equipment, without a fake Line
//! or remote execution service. The selected remote Host is a CPU fixture.
use super::member_leave::{
    admit_with_key, owner_fixture, owner_fixture_with_host, owner_fixture_with_plot,
};
use super::*;
use conduit_body::BodyState;

fn minimal(composition: StdHostComposition) -> StdHost {
    StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/owner-test".into(),
            boot_id: "boot/member-planning-owner".into(),
            offer_generation: OfferGeneration(1),
        },
        composition,
    )
}

#[test]
fn authenticated_remote_offers_can_supply_an_exact_unstarted_body_plan() {
    let (mut owner, root) = owner_fixture_with_host(minimal(StdHostComposition::minimal()));
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let checked = source.expand_entry_for_authoring().unwrap();
    let before = owner.session.evidence().clone();
    assert!(owner.plan_with_source(&source, &checked).is_err());
    assert_eq!(owner.session.evidence(), &before);
    let (credential, _, remote) = admit_with_key(&mut owner, &root, "host/remote-equipment");
    owner.plan_with_source(&source, &checked).unwrap();
    let proposal = owner.session.realization().unwrap().clone();
    let [partition] = proposal.plan.plots.as_slice() else {
        panic!("one Plot")
    };
    let [fragment] = partition.plan.fragments.as_slice() else {
        panic!("one remote fragment")
    };
    assert_eq!(fragment.host_id, remote.host_id);
    assert_eq!(fragment.boot_id, remote.boot_id);
    assert_eq!(fragment.offer_generation, remote.offer_generation);
    assert!(fragment.placements.iter().all(|placement| {
        placement.host_id == remote.host_id
            && placement.boot_id == remote.boot_id
            && placement.offer_generation == remote.offer_generation
    }));
    assert!(proposal.play.is_none());
    assert_eq!(
        owner.execute_reference_fixture(1000).unwrap_err(),
        "remote-fragment-preparation-unavailable"
    );
    assert_eq!(owner.session.realization(), Some(&proposal));
    assert!(owner.last_execution.is_none());
    assert!(!owner.host.is_playing());
    owner.lull().unwrap();
    owner.member_leave(&root, &credential).unwrap();
    let before = owner.session.evidence().clone();
    assert!(owner.plan_with_source(&source, &checked).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert!(owner.session.realization().is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn observed_members_preserve_local_selection_and_actual_local_execution() {
    let (mut owner, root) = owner_fixture();
    // A lexically earlier Host must not displace the ordinary local default.
    admit_with_key(&mut owner, &root, "aaa/remote-equipment");
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let checked = source.expand_entry_for_authoring().unwrap();
    owner.plan_with_source(&source, &checked).unwrap();
    let proposal = owner.session.realization().unwrap();
    assert!(proposal
        .plan
        .plots
        .iter()
        .flat_map(|plot| &plot.plan.fragments)
        .all(|fragment| fragment.host_id == owner.host.advertisement().host_id));
    owner.execute_reference_fixture(1000).unwrap();
    assert!(owner.last_execution.as_ref().unwrap()["output_utf8"]
        .as_str()
        .unwrap()
        .contains("Hello."));
    assert_eq!(owner.session.evidence().body.state, BodyState::Lulled);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn member_offers_do_not_invent_lines_for_cross_host_cords() {
    let nodes = (0..17)
        .map(|i| format!("n{i:02}: logic/not()\n"))
        .collect::<String>();
    let chain = (0..17)
        .map(|i| format!("n{i:02}"))
        .collect::<Vec<_>>()
        .join(" >> ");
    let source = crate::plot_source::parse(&format!(
        "plot remote-view (\n >> value: Boolean\n result: Boolean >>\n) {{\n{nodes}value >> {chain} >> result\n}}"
    )).unwrap();
    let checked = source.expand_entry_for_authoring().unwrap();
    let (mut owner, root) = owner_fixture_with_plot(
        minimal(StdHostComposition::minimal().with_logic()),
        &checked,
    );
    // The seventeenth operation cannot use the local Back's sixteen instances.
    let local = owner
        .host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == "logic/not")
        .unwrap();
    assert_eq!(local.limits.max_active_instances, 16);
    admit_with_key(&mut owner, &root, "host/remote-logic");
    let placements =
        conduit_planner::default_expanded_placements(&checked.expanded, &owner.planning_hosts())
            .unwrap();
    assert!(placements
        .by_gear
        .values()
        .any(|choice| choice.host_id == owner.host.advertisement().host_id));
    assert!(placements
        .by_gear
        .values()
        .any(|choice| choice.host_id.as_str() == "host/remote-logic"));
    let before = owner.session.evidence().clone();
    // The checked chain crosses Hosts and needs an actual selected Line.
    let error = owner.plan_with_source(&source, &checked).unwrap_err();
    assert!(error.contains("Line"), "{error}");
    assert_eq!(owner.session.evidence(), &before);
    assert!(owner.session.realization().is_none());
    assert!(owner.last_execution.is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn owner_defaults_fit_narrow_backs_without_relaxing_explicit_queue_requirements() {
    let source = crate::plot_source::parse(
        "plot bounded-value {\n source: scalar/literal(value = 1)\n clamp: math/clamp(minimum = 0, maximum = 2)\n source.value >> clamp.in\n}",
    ).unwrap();
    let checked = source.expand_entry_for_authoring().unwrap();
    let (mut owner, root) = owner_fixture_with_plot(host("boot/bounded-queues"), &checked);
    let hosts = [owner.host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&checked.expanded, &hosts).unwrap();
    let strict = conduit_planner::plan_expanded_authoring_with_options(
        &checked,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &Default::default(),
            line_candidates: &Default::default(),
            connection_item_capacity: 4,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &Default::default(),
    );
    assert!(matches!(
        strict,
        Err(conduit_planner::PlannerError::QueueRequirementAboveHostLimit(_))
    ));
    owner.plan_with_source(&source, &checked).unwrap();
    let connections = &owner.session.realization().unwrap().plan.plots[0]
        .plan
        .fragments[0]
        .connections;
    assert!(!connections.is_empty());
    assert!(connections
        .iter()
        .all(|cord| cord.item_capacity == 1 && cord.byte_capacity == 8));
    // This admission-only Plot leaves its result unconnected. Execution proof
    // below supplies an explicit acknowledging typed Fore consumer instead.
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn prepared_scalar_math_completes_one_body_play_through_an_acknowledged_fore() {
    use conduit_core::{port_id, ConnectionTrack, PortDirection, Scalar};
    use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
    use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
    struct Output(Option<Scalar>);
    impl BodyForeOutputAdapter for Output {
        fn deliver(
            &mut self,
            output: &conduit_std_host::ExternalForeDelivery,
        ) -> Result<(), String> {
            if self.0.is_some() {
                return Err("one result only".into());
            }
            self.0 = Some(Scalar::decode(&output.bytes).map_err(|error| format!("{error:?}"))?);
            Ok(())
        }
    }
    struct Timer;
    impl conduit_std_host::TimerAdapter for Timer {
        fn wait(&mut self, _: std::time::Duration) {}
    }
    // These startup I64 fields use canonical raw Scalar microunits.
    for (operation, expected) in [
        ("math/clamp(minimum = 0, maximum = 1000000)", Scalar::ONE),
        (
            "math/scale(gain = 3000000)",
            Scalar::from_raw_microunits(6 * Scalar::SCALE),
        ),
        ("math/deadband(radius = 3000000)", Scalar::ZERO),
    ] {
        let source = crate::plot_source::parse(&format!(
            "plot scalar-result (\n result: Scalar >>\n) {{\n source: scalar/literal(value = 2000000)\n transform: {operation}\n source.value >> transform.in\n transform.out >> result\n}}"
        )).unwrap();
        let checked = source.expand_entry_for_authoring().unwrap();
        let (mut owner, root) = owner_fixture_with_plot(host("boot/scalar-body"), &checked);
        let ad = owner.host.advertisement().clone();
        let hosts = [ad.clone()];
        let placements =
            conduit_planner::default_expanded_placements(&checked.expanded, &hosts).unwrap();
        let queues = conduit_planner::default_expanded_connection_limits(
            &checked.expanded,
            &hosts,
            &placements,
        )
        .unwrap();
        let boundary = std::collections::BTreeMap::from([(
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("result"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 8,
            },
        )]);
        let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
            &checked,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            PlanningOptions {
                connection_bases: &Default::default(),
                line_candidates: &Default::default(),
                connection_item_capacity: 1,
                connection_byte_capacity: 8,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &queues,
            &boundary,
        )
        .unwrap();
        owner
            .session
            .propose(
                vec![conduit_body::BodyPlotPlan {
                    plot: resident(&checked),
                    plan,
                }],
                &ad.host_id,
                &ad.boot_id,
            )
            .unwrap();
        let proposal = owner.session.realization().unwrap().clone();
        let mut output = Output(None);
        let report = owner
            .host
            .run_body_plan_with_fore_to(
                BodyRunRequest {
                    wake: &proposal.wake,
                    plan: &proposal.plan,
                    control: &conduit_std_host::RunControl::default(),
                    keyboard: None,
                },
                &[],
                false,
                &mut output,
                &mut Vec::with_capacity(256),
                &mut Timer,
            )
            .unwrap();
        assert_eq!(
            report.terminal,
            conduit_core::TerminalDisposition::Completed,
            "{operation}: {:?}",
            report.failure
        );
        assert_eq!(report.play.body_id, owner.session.evidence().body_id);
        assert_eq!(report.play.plan_id, proposal.plan.plan_id);
        assert_eq!(report.play.wake_id, proposal.wake.wake_id);
        assert_eq!(output.0, Some(expected), "{operation}");
        assert_eq!(report.requests.len(), 1);
        owner
            .session
            .started(
                &ad.host_id,
                &ad.boot_id,
                report.play.clone(),
                report.wake_at_start,
            )
            .unwrap();
        owner
            .session
            .lull(&ad.host_id, &ad.boot_id, Some(&report.play))
            .unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn remote_projection_uses_retained_proposal_and_current_member_without_starting() {
    use std::cell::Cell;
    let (mut owner, root) = owner_fixture_with_host(minimal(StdHostComposition::minimal()));
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let checked = source.expand_entry_for_authoring().unwrap();
    let (credential, _, remote) = admit_with_key(&mut owner, &root, "host/remote-projection");
    let called = Cell::new(false);
    assert!(owner
        .prepare_remote_body_workset(&remote, 4096, |_| {
            called.set(true);
            Err(conduit_embedded_build::BodyFragmentGenerationRefusal::Resources)
        })
        .is_err());
    assert!(
        !called.get(),
        "no retained proposal must refuse before resolving"
    );
    owner.plan_with_source(&source, &checked).unwrap();
    let before = owner.session.evidence().clone();
    let proposal = owner.session.realization().unwrap().clone();
    let prospective = owner
        .host
        .prospective_body_play(&proposal.wake, &proposal.plan)
        .unwrap();
    let refused = owner
        .prepare_remote_body_workset(&remote, 4096, |scope| {
            called.set(true);
            assert_eq!(scope.plan(), &proposal.plan);
            assert_eq!(scope.play(), &prospective);
            assert_eq!(scope.fragment().host_id, remote.host_id);
            assert_eq!(
                scope.fragment().source_document_id,
                checked.expanded.source_document_id
            );
            // This fixture has no selected native implementation registry. Refuse
            // rather than turn the ordinary hosted Face provider into an MCU Back.
            Err(conduit_embedded_build::BodyFragmentGenerationRefusal::Resources)
        })
        .err()
        .unwrap();
    assert!(called.get());
    assert!(refused.contains("Resources"));
    let unresolved = owner
        .prepare_remote_resident_workset(
            &remote,
            &[],
            conduit_embedded_build::BodyFragmentProjectionLimits {
                image: conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
                computations: conduit_embedded_build::EmbeddedComputationBounds {
                    maximum_program_bytes: 512 * 1024,
                    maximum_total_program_bytes: 1024 * 1024,
                },
                assigned: conduit_core::AssignedPlanMaxima {
                    encoded_bytes: 4096,
                    runtime_state_bytes: 16384,
                    counts: [32; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
                },
                maximum_fore_ports: 2,
                maximum_document_bytes: 4096,
            },
        )
        .err()
        .unwrap();
    assert!(unresolved.contains("Resident(Selection)"));
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.session.realization(), Some(&proposal));
    assert_eq!(
        owner
            .host
            .prospective_body_play(&proposal.wake, &proposal.plan)
            .unwrap(),
        prospective
    );
    assert!(!owner.host.is_playing());
    assert!(owner.last_execution.is_none());
    called.set(false);
    let mut stale = remote.clone();
    stale.offer_generation.0 += 1;
    assert!(owner
        .prepare_remote_body_workset(&stale, 4096, |_| {
            called.set(true);
            panic!("unobserved member reached resolver")
        })
        .is_err());
    assert!(!called.get());
    owner.lull().unwrap();
    owner.member_leave(&root, &credential).unwrap();
    assert!(owner
        .prepare_remote_body_workset(&remote, 4096, |_| {
            panic!("withdrawn member reached resolver")
        })
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
