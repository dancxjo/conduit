//! The default path must preserve the former owned planning projection exactly.
use super::*;
use crate::PlanningOptions;
#[test]
fn borrowed_default_preserves_plan_spans_completion_and_refusals() {
    let syntax = conduit_plot::parse_syntax_document(
        "plot signal-demo {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n show: face/show(\"Signal\")\n pulse >> show\n}.\n",
    );
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_signal::signal_startup_catalog())
            .unwrap();
    let expanded = conduit_plot::expand_canonical_plot(
        &checked,
        "signal-demo",
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap();
    let original = expanded.clone();
    let hosts = [crate::tests::host()];
    let placements = default_expanded_placements(&expanded, &hosts).unwrap();
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    for queue_items in [0, 1, u16::MAX] {
        let connection_bases = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let options = PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: queue_items,
            connection_byte_capacity: conduit_signal::SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let legacy = crate::plan_validated_plot(
            &expanded_planning_plot(&expanded),
            &hosts,
            &placements,
            &bases,
            options,
        )
        .and_then(|mut plan| {
            attach_source_spans(&expanded.provenance, &mut plan)?;
            Ok(
                conduit_core::seal_plan_with_realization_backs_and_completion(
                    PlotIdentity {
                        source_document_id: expanded.source_document_id.clone(),
                        checked_plot_id: expanded.checked_plot_id.clone(),
                        expanded_plot_id: expanded.expanded_plot_id.clone(),
                    },
                    crate::plan_completion_policy(expanded.completion),
                    expanded.realization_backs.clone(),
                    plan.fragments,
                ),
            )
        });
        let current =
            plan_expanded_canonical_with_options(&expanded, &hosts, &placements, &bases, options);
        let consumed = plan_expanded_canonical_consuming_with_connection_limits(
            expanded.clone(),
            &hosts,
            &placements,
            &bases,
            options,
            &BTreeMap::new(),
        );
        assert_eq!(current, consumed);
        #[cfg(feature = "indexed-source")]
        {
            let bytes = conduit_plot::flat_image::indexed::encode(&expanded).unwrap();
            // SAFETY: this encoder just validated the same checked Source.
            let image = unsafe {
                conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes)
            }
            .unwrap();
            let indexed = crate::indexed_source::plan_with_connection_limits(
                &image,
                &hosts,
                &placements,
                &bases,
                options,
                &BTreeMap::new(),
            );
            assert_eq!(current, indexed);
        }
        assert_eq!(current, legacy);
        if queue_items == 1 {
            let plan = current.unwrap();
            assert!(conduit_core::verify_plan(&plan));
            assert_eq!(
                plan.completion_policy,
                conduit_core::PlanCompletionPolicy::SemanticCompletion
            );
            assert!(plan
                .fragments
                .iter()
                .flat_map(|f| &f.placements)
                .all(|p| p.source_span.is_some()));
        } else {
            assert!(current.is_err());
        }
        assert_eq!(expanded, original);
    }
}

#[cfg(feature = "indexed-source")]
#[test]
fn retiring_advertisements_preserve_repeated_capabilities_and_queue_refusals() {
    let syntax = conduit_plot::parse_syntax_document(
        "plot repeated {\n first: flow/pulse(count = 2, period-ms = 0, initial = false)\n second: flow/pulse(count = 2, period-ms = 0, initial = false)\n a: face/show(\"A\")\n b: face/show(\"B\")\n first >> a\n second >> b\n}.\n",
    );
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_signal::signal_startup_catalog())
            .unwrap();
    let expanded = conduit_plot::expand_canonical_plot(
        &checked,
        "repeated",
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap();
    let mut host = crate::tests::host();
    let mut unused = host.capabilities[0].clone();
    unused.capability_id = "unused/pulse".into();
    host.capabilities.push(unused);
    let hosts = [host];
    let placements = default_expanded_placements(&expanded, &hosts).unwrap();
    assert!(placements
        .by_gear
        .values()
        .all(|p| p.capability_id.as_str() != "unused/pulse"));
    let bytes = conduit_plot::flat_image::indexed::encode(&expanded).unwrap();
    // SAFETY: the checked Make encoder produced this exact source image.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let queues = BTreeMap::new();
    for items in [0, 1, u16::MAX] {
        let options = PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: items,
            connection_byte_capacity: conduit_signal::SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let expected =
            plan_expanded_canonical_with_options(&expanded, &hosts, &placements, &bases, options);
        let actual = crate::indexed_source::plan_compact_consuming_with_connection_limits(
            &image,
            hosts.to_vec(),
            &placements,
            &bases,
            options,
            &queues,
        )
        .and_then(|plan| plan.materialize());
        assert_eq!(actual, expected);
        assert_eq!(actual.is_ok(), items == 1);
    }
}
