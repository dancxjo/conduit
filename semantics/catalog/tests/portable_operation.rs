#![cfg(feature = "kernel-step")]
//! A non-environmental authored computation uses shared preparation directly.
use conduit_core::{ConfigurationValue, PortTemporal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    PortableOperationBack, PortableOperationRefusal, PreparedPortableOperation,
};

fn gear() -> conduit_plot::CheckedGear {
    let checked = check_syntax_document(
        &parse_syntax_document("plot increment (\n input: U8 >> result: U8\n) = (. + 1)\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    expand_canonical_plot_for_authoring(&checked, "increment", &ProfileCatalog::new())
        .unwrap()
        .expanded
        .gears
        .remove(0)
}
#[test]
fn authored_increment_prepares_and_runs_without_application_defaults() {
    let prepared =
        PreparedPortableOperation::prepare(&gear(), PortTemporal::Flow { closes: true }, 16, 16)
            .unwrap();
    assert!(prepared.host_calls.is_empty());
    assert!(prepared.selector.is_none());
    let PortableOperationBack::Expression(mut back) = prepared.back else {
        panic!("expression")
    };
    let mut io = StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        })],
        [false],
        [Some(16)],
        None,
        16,
    );
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&[41])], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        <conduit_semantic_catalog::PreparedExpressionBack as StepBack<1>>::prepared_output(
            &back,
            PortId(0)
        ),
        Some(&[42][..])
    );
}
#[test]
fn missing_or_malformed_configuration_refuses() {
    let mut gear = gear();
    gear.configuration.clear();
    assert!(matches!(
        PreparedPortableOperation::prepare(&gear, PortTemporal::Flow { closes: true }, 16, 16),
        Err(PortableOperationRefusal::Unsupported)
    ));
    gear.configuration.push(conduit_core::ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text("malformed".into()),
    });
    assert!(matches!(
        PreparedPortableOperation::prepare(&gear, PortTemporal::Flow { closes: true }, 16, 16),
        Err(PortableOperationRefusal::Initialization)
    ));
}

#[test]
fn shared_offer_custody_retains_arbitrary_initialized_payload_and_current_epoch() {
    use conduit_core as c;
    use conduit_semantic_catalog::{ReadyBacks, ReadyOperation, SoftwareRefusal};
    let gear = gear();
    let prepared =
        PreparedPortableOperation::prepare(&gear, PortTemporal::Flow { closes: true }, 16, 16)
            .unwrap();
    let offer = c::BackOfferBuilder::new(
        prepared.kind,
        c::Back {
            capability_id: "increment/back".into(),
            execution_profile_id: "test/prepared@1".into(),
            implementation_id: "increment/implementation".into(),
            artifact_id: "increment/artifact".into(),
            host_calls: prepared.host_calls,
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let mut owner = ReadyBacks::new(
        c::PlotIdentity {
            source_document_id: "increment/source".into(),
            checked_plot_id: "increment/checked".into(),
            expanded_plot_id: "increment/expanded".into(),
        },
        "increment/host".into(),
        "increment/boot".into(),
        vec![ReadyOperation {
            gear: gear.gear_id,
            configuration: gear.configuration,
            offer: offer.clone(),
            prepared: Some(7u8),
        }],
    );
    assert_eq!(
        owner.release_transferred_metadata(),
        Err(SoftwareRefusal::Selection)
    );
    let host = "increment/host".into();
    let boot = "increment/boot".into();
    {
        let offers = owner
            .current_offers(&host, &boot, c::OfferGeneration(2))
            .unwrap();
        assert_eq!(offers.source().as_str(), "increment/source");
        assert_eq!(offers.iter().collect::<Vec<_>>(), vec![&offer]);
    }
    for (host, boot, generation) in [
        ("other/host", "increment/boot", 2),
        ("increment/host", "other/boot", 2),
        ("increment/host", "increment/boot", 1),
        ("increment/host", "increment/boot", 0),
    ] {
        assert!(matches!(
            owner.current_offers(&host.into(), &boot.into(), c::OfferGeneration(generation)),
            Err(SoftwareRefusal::Advertisement)
        ));
    }
    let offers = owner
        .current_offers(&host, &boot, c::OfferGeneration(2))
        .unwrap();
    assert_eq!(offers.iter().count(), 1);
}
