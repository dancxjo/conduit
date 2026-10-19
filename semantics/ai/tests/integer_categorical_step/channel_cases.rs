//! The existing production composite kernel drives the real categorical factory.
//! No test Back, scheduler replacement, or manual step_committed call is used.
use super::*;
use conduit_ai::integer_categorical_step::channel::*;
use conduit_composite::KernelCompositeDefinition;

fn channel_definition(
    profile: &PreparedCategoricalStep,
) -> (conduit_plot::CheckedPlot, KernelCompositeDefinition) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut catalog, true).unwrap();
    let input = profile.indices_type().profile().unwrap();
    let output = profile.scores_type().profile().unwrap();
    let source = format!(
        "plot categorical-channel (\n >> input: {}...|\n output: {}...| >>\n) {{\n model: {}\n input >> model.indices\n model.scores >> output\n}}\n",
        input.value_kind().as_str(), output.value_kind().as_str(), profile.kind_identity(true));
    let plot = conduit_plot::parse_with_startup(&source, &startup, &catalog).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "categorical-test".into(),
        boot_id: "categorical-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/host@1".into(),
        bases: vec![],
        resources: vec![profile.resource_offer().clone()],
        planner_capabilities: vec![],
        capabilities: vec![profile.offer(true).unwrap()],
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(profile.indices_type())
        .unwrap()
        .max(conduit_plot::maximum_prepared_canonical_value_bytes(profile.scores_type()).unwrap());
    let plan = conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        1,
        maximum,
    )
    .unwrap();
    let definition = KernelCompositeDefinition::from_authored_export(
        "categorical-channel".into(),
        "categorical-channel-boot".into(),
        OfferGeneration(1),
        "categorical/channel@1".into(),
        "categorical/channel@1".into(),
        "categorical/channel-artifact@1".into(),
        &plot,
        &"run".into(),
        plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .unwrap();
    (plot, definition)
}
fn poll<L>(channel: &mut PreparedCategoricalChannel<L>) -> CompletedCategoricalRequest<L> {
    // Test harness bound; production channel performs exactly one kernel step per poll.
    for _ in 0..16 {
        if let Some(done) = channel.step().unwrap() {
            return done;
        }
    }
    panic!("production kernel did not complete bounded numerical request")
}

#[test]
fn ordinary_kernel_channel_keeps_distinct_leases_for_equal_inputs_and_preflights_pressure() {
    let profile = profile();
    let (plot, definition) = channel_definition(&profile);
    let mut channel =
        PreparedCategoricalChannel::prepare(&plot, definition.clone(), profile.clone(), 1).unwrap();
    assert_eq!(channel.active_plays().len(), 1);
    let bytes = indices(&profile, &[1, 1]);
    assert_eq!(
        channel.submit("branch-a", &bytes).unwrap(),
        CategoricalChannelSubmission::Accepted
    );
    assert_eq!(channel.pending(), Some(&"branch-a"));
    let busy = channel.submit("branch-b", &bytes).unwrap_err();
    assert_eq!(busy.refusal, CategoricalChannelRefusal::Busy);
    assert_eq!(busy.lease, "branch-b");
    let (lease, receipt) = poll(&mut channel).into_parts();
    assert_eq!(lease, "branch-a");
    assert_eq!(receipt.invocation(), 1);
    assert_eq!(receipt.active_plays(), channel.active_plays());
    let mut actual = vec![0; receipt.output_len()];
    receipt.copy_output_into(&mut actual).unwrap();
    assert_eq!(scores(&actual), [4, -4]);
    let pressure = channel.submit(busy.lease, &bytes).unwrap_err();
    assert_eq!(pressure.refusal, CategoricalChannelRefusal::Pressure);
    assert_eq!(pressure.lease, "branch-b");
    assert!(channel.pending().is_none());
    assert!(!channel.is_closed());
    drop(receipt);
    assert_eq!(
        channel.submit(pressure.lease, &bytes).unwrap(),
        CategoricalChannelSubmission::Accepted
    );
    let (lease, receipt) = poll(&mut channel).into_parts();
    assert_eq!(lease, "branch-b");
    assert_eq!(receipt.invocation(), 2);
    assert_eq!(receipt.plan(), &definition.internal_plan);
    assert_eq!(channel.pending(), None);
}

#[test]
fn ordinary_kernel_channel_refuses_foreign_definition_and_keeps_cancelled_pending_lease() {
    let profile = profile();
    let (plot, definition) = channel_definition(&profile);
    let mut foreign = definition.clone();
    foreign.boundary.input_fronts[0].internal_port_id = "foreign".into();
    assert!(matches!(
        PreparedCategoricalChannel::<()>::prepare(&plot, foreign, profile.clone(), 1),
        Err(CategoricalChannelRefusal::Definition)
    ));
    let mut channel =
        PreparedCategoricalChannel::prepare(&plot, definition, profile.clone(), 1).unwrap();
    let malformed = channel.submit("malformed", &[]).unwrap_err();
    assert_eq!(malformed.refusal, CategoricalChannelRefusal::Input);
    assert_eq!(malformed.lease, "malformed");
    assert!(channel.pending().is_none());
    let bytes = indices(&profile, &[0, 2]);
    assert_eq!(
        channel.submit("cancelled", &bytes).unwrap(),
        CategoricalChannelSubmission::Accepted
    );
    channel.cancel().unwrap();
    assert!(channel.is_closed());
    assert!(matches!(
        channel.step(),
        Err(CategoricalChannelRefusal::Closed)
    ));
    let refused = channel.submit("after-close", &bytes).unwrap_err();
    assert_eq!(refused.refusal, CategoricalChannelRefusal::Closed);
    assert_eq!(refused.lease, "after-close");
    let pending = match channel.into_pending() {
        Ok(value) => value,
        Err(_) => panic!("closed channel"),
    };
    let (lease, receipt) = pending.unwrap();
    assert_eq!(lease, "cancelled");
    assert!(receipt.is_none());
}
