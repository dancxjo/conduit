//! Exact checked Source correspondence is stronger than a fresh Plan digest.
use conduit_composite::KernelCompositeHost;
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
#[path = "canonical_export/fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn canonical_wide_payload_crosses_real_kernel_cord_and_independent_fores() {
    let fixture = Fixture::new();
    let plan = &fixture.definition.internal_plan;
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &fixture.document,
        ENTRY,
        &fixture.catalog,
    )
    .unwrap();
    assert_eq!(
        plan.source_document_id,
        authored.expanded.source_document_id
    );
    assert_eq!(plan.checked_plot_id, authored.expanded.checked_plot_id);
    assert_eq!(plan.expanded_plot_id, authored.expanded.expanded_plot_id);
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 2);
    assert_eq!(plan.fragments[0].connections.len(), 1);
    assert_eq!(plan.fragments[0].fore_ports.len(), 2);
    for fore in &plan.fragments[0].fore_ports {
        assert_eq!(fore.item_capacity, 2);
        assert_eq!(fore.byte_capacity, BYTES);
        assert_eq!(fore.value_contract.as_ref().unwrap().maximum_bytes, BYTES);
    }
    let input = ValuePayload {
        value_kind: kind_id(BYTES_INFO_ID),
        encoded: (0..=255).collect(),
    };
    assert_eq!(input.encoded.len(), BYTES as usize);
    assert!(input.encoded.len() > 64);
    let mut output = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: Vec::with_capacity(BYTES as usize),
    };
    let mut host = KernelCompositeHost::prepare(fixture.definition, &registry()).unwrap();
    host.start().unwrap();
    let plays = host.active_plays().clone();
    assert_eq!(plays.len(), 1);
    assert!(matches!(
        host.admit_input(&port_id("input"), 0, &input).unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    ));
    let mut completed = false;
    for _ in 0..16 {
        host.step().unwrap();
        if let Some(sequence) = host.output_into(&port_id("output"), &mut output).unwrap() {
            assert_eq!(sequence, 0);
            host.complete_output(&port_id("output"), sequence).unwrap();
            completed = true;
            break;
        }
    }
    assert!(
        completed,
        "admitted generic request must complete in the finite observed turn bound"
    );
    assert_eq!(output, input);
    assert_eq!(host.active_plays(), &plays);
    assert!(!host.signs().is_empty());
    host.cancel().unwrap();
}

#[test]
fn foreign_source_identity_and_freshly_sealed_fores_and_graphs_refuse() {
    let fixture = Fixture::new();
    let mut foreign = fixture.document.clone();
    foreign.source_document_id = "source/foreign-document".into();
    assert!(rebuild(
        &foreign,
        &fixture.catalog,
        fixture.definition.internal_plan.clone()
    )
    .is_err());
    let mut valid_fore = 0;
    let mut valid_graph = 0;
    for alteration in 0..19 {
        let mut plan = fixture.definition.internal_plan.clone();
        match alteration {
            0 => plan.source_document_id = "source/foreign".into(),
            1 => plan.checked_plot_id = "checked/foreign".into(),
            2 => plan.expanded_plot_id = "expanded/foreign".into(),
            3 => {
                plan.fragments[0].fore_ports.pop();
            }
            4 => {
                let fore = plan.fragments[0].fore_ports[0].clone();
                plan.fragments[0].fore_ports.push(fore);
            }
            5 => plan.fragments[0].fore_ports[0].byte_capacity = 1,
            6 => plan.fragments[0].fore_ports[0].byte_capacity = BYTES + 1,
            7 => {
                plan.fragments[0].fore_ports[0]
                    .value_contract
                    .as_mut()
                    .unwrap()
                    .maximum_bytes = 128
            }
            8 => plan.fragments[0].fore_ports[0].track = ConnectionTrack::NormalClose,
            9 => plan.fragments[0].placements[0].gear_id = "forged/gear".into(),
            10 => plan.fragments[0].placements[0].kind_id = "forged/kind".into(),
            11 => plan.fragments[0].placements[0]
                .configuration
                .push(ConfigurationEntry {
                    key: "forged".into(),
                    value: ConfigurationValue::Text("not-authored".into()),
                }),
            12 => {
                plan.fragments[0].connections.pop();
            }
            13 => {
                let mut cord = plan.fragments[0].connections[0].clone();
                cord.connection_id = "forged/extra-cord".into();
                plan.fragments[0].connections.push(cord);
            }
            14 => plan.fragments[0].connections[0].track = ConnectionTrack::NormalClose,
            15 => plan.fragments[0].connections[0].byte_capacity = BYTES + 1,
            16 => plan.fragments[0].fore_ports[0].gear_port_id = port_id("out"),
            17 => plan.fragments[0].connections[0].sink_port_id = port_id("missing"),
            18 => plan.fragments[0].connections[0].byte_capacity = BYTES - 1,
            _ => unreachable!(),
        }
        let plan = reseal(plan);
        if alteration == 18 {
            assert!(
                verify_plan(&plan),
                "underbound Cord remains integrity-valid; typed correspondence must refuse it"
            );
        }
        if verify_plan(&plan) {
            if (3..=8).contains(&alteration) {
                valid_fore += 1;
            }
            if (9..=18).contains(&alteration) {
                valid_graph += 1;
            }
        }
        assert!(
            fixture.rebuild(plan).is_err(),
            "resealed forgery {alteration}"
        );
    }
    assert!(
        valid_fore > 0,
        "Fore correspondence must reject at least one integrity-valid counterexample"
    );
    assert!(
        valid_graph > 0,
        "graph correspondence must reject at least one integrity-valid counterexample"
    );
}

#[test]
fn unsupported_zero_oversize_and_abnormal_queue_profiles_refuse() {
    let fixture = Fixture::new();
    for alteration in 0..4 {
        let mut plan = fixture.definition.internal_plan.clone();
        match alteration {
            0 => plan.fragments[0].fore_ports[0].item_capacity = 0,
            1 => plan.fragments[0].fore_ports[0].item_capacity = 3,
            2 => {
                plan.fragments[0].fore_ports[0].abnormal_kind = Some(kind_id("failure/unsupported"))
            }
            3 => plan.fragments[0].connections[0].byte_capacity = 0,
            _ => unreachable!(),
        }
        assert!(
            fixture.rebuild(reseal(plan)).is_err(),
            "unsupported profile {alteration}"
        );
    }
}

#[test]
fn legitimate_finite_queue_allowances_are_plan_facts_not_source_forgeries() {
    let fixture = Fixture::new();
    let mut plan = fixture.definition.internal_plan.clone();
    for fore in &mut plan.fragments[0].fore_ports {
        fore.item_capacity = 1;
    }
    plan.fragments[0].connections[0].item_capacity = 1;
    let plan = reseal(plan);
    assert!(verify_plan(&plan));
    let rebound = fixture.rebuild(plan.clone()).unwrap();
    assert_eq!(rebound.internal_plan, plan);
    assert_eq!(rebound.external_capability.limits.max_queue_items, 1);
}

#[test]
fn checked_input_fore_fanout_is_explicitly_outside_the_initial_profile() {
    let source = source().replace("first.out >> second.in", "input >> second.in");
    let (document, catalog, plan) = Fixture::prepare_source(&source);
    assert!(verify_plan(&plan));
    let error = rebuild(&document, &catalog, plan).unwrap_err();
    assert!(error.to_string().contains("missing/extra/fanout Fore"));
}
