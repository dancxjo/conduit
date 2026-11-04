//! Independently installed current provider custody, not planner-supplied truth.
use super::*;
fn checker() -> PreparedSourceChecker {
    let (mut owner, request) = support::owner(VALID, 7);
    PreparedSourceChecker::prepare(
        &mut [SourceCheckPreparation {
            owner: &mut owner,
            request: &request,
        }],
        bounds(),
    )
    .unwrap()
}
#[test]
fn current_provider_loss_generation_boot_and_resource_refuse_old_source_check_plan() {
    let prepared = checker();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    prepared
        .install_authoring(&mut startup, &mut profiles)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(CHECK), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "check-source", &profiles).unwrap();
    let mut original = StdHost::new();
    let definition = support::definition(original.advertisement(), &prepared);
    let grant = definition.grant.clone();
    // SAFETY: Actual bounded prepared checker and independent current definition.
    unsafe {
        original.install_immediate_native_source(definition, Box::new(prepared.into_provider()))
    }
    .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
    let plan = support::plan(&original, &authored, &[grant], bounds()).unwrap();
    let (_, request) = support::owner(VALID, 7);
    let bytes = SourceCheckNativeSchema::prepare()
        .unwrap()
        .encode_request(&request, bounds().request_bytes as usize)
        .unwrap();
    for change in 0..5 {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: original.advertisement().host_id.clone(),
            boot_id: if change == 2 {
                "fixture/restarted-source-host".into()
            } else {
                original.advertisement().boot_id.clone()
            },
            // Installation advances the offer generation once. Keep it equal
            // to the old Plan so the intended provider/Boot mismatch is isolated.
            offer_generation: OfferGeneration(
                original.advertisement().offer_generation.0 - u64::from(change != 0),
            ),
        });
        if change != 0 {
            let prepared = checker();
            let mut definition = support::definition(host.advertisement(), &prepared);
            if change == 1 {
                definition.base.provider_generation += 1;
            }
            if change == 3 {
                definition.base.resources[0].pool_id = "fixture/replaced-source-backing".into();
            }
            // SAFETY: New independent provider; old Plan cannot choose its generation.
            unsafe {
                host.install_immediate_native_source(definition, Box::new(prepared.into_provider()))
            }
            .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
        }
        let mut capture = Capture {
            bytes: vec![],
            calls: 0,
        };
        let outcome = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: bytes.clone(),
            }],
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
        );
        if change == 4 {
            // Positive control: identical current custody accepts the same Plan.
            outcome.unwrap();
            assert_eq!(capture.calls, 1);
            assert!(!capture.bytes.is_empty());
        } else {
            assert!(outcome.is_err(), "change {change}");
            assert_eq!(capture.calls, 0);
            assert!(capture.bytes.is_empty());
        }
    }
}

#[test]
fn preparation_stop_refuses_before_specialization_and_preserves_source_owner() {
    let (mut owner, request) = support::owner(VALID, 7);
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("cancel-source-preparation").unwrap())
        .unwrap();
    let (_, cancelled_cost) = allocation::allocation_probe::observe(|| {
        let error = PreparedSourceChecker::prepare_controlled(
            &mut [SourceCheckPreparation {
                owner: &mut owner,
                request: &request,
            }],
            bounds(),
            &control,
        )
        .err()
        .expect("cancelled preparation cannot return a provider");
        assert_eq!(error, "Source-check preparation cancelled");
    });
    // Only the refusal string is allocated inside this preparation call.
    // The already authorized Source/context owner was constructed beforehand.
    assert_eq!(cancelled_cost.allocations, 1);
    assert_eq!(cancelled_cost.reallocations, 0);
    assert_eq!(owner.snapshot(), &request.snapshot);
    assert_eq!(owner.context(), &request.context);
    let fresh_control = RunControl::default();
    let prepared = PreparedSourceChecker::prepare_controlled(
        &mut [SourceCheckPreparation {
            owner: &mut owner,
            request: &request,
        }],
        bounds(),
        &fresh_control,
    )
    .unwrap();
    assert!(prepared.retained_bytes() <= bounds().retained_bytes);
}
