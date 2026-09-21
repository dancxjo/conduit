//! Prepared browser realization checks; these do not claim live human input.
use super::*;
use crate::installed_browser::BROWSER_PORTS_PER_GEAR;
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::ValueStorage;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome, PortId, RequestId};
use std::collections::BTreeMap;

fn placements() -> Vec<PlannedGear> {
    let (startup, profile) = crate::installed_browser::catalogs().unwrap();
    let syntax = parse_syntax_document("form timing {\n derive: time/ordered-event-intervals\n normalize: sequence/normalize-relative-duration\n derive.intervals > normalize.intervals\n}\n");
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "timing", &profile).unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "timing-browser".into(),
        "timing-boot".into(),
    )];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0)
    .placements
}

#[test]
fn browser_plans_prepare_shared_codecs_and_emit_exact_timing_values() {
    let placements = placements();
    let events =
        conduit_semantic_catalog::timed_event_sequence_value("fixture/clock", &[100, 200, 500])
            .unwrap()
            .canonical_bytes()
            .unwrap();
    let mut input = events;
    for (index, implementation) in IMPLEMENTATIONS.iter().enumerate() {
        let placement = placements
            .iter()
            .find(|p| p.implementation_id.as_str() == *implementation)
            .unwrap();
        let mut codec = PreparedTiming::for_placement(placement).unwrap().unwrap();
        let mut store = HostedValueStore::new(4, MAXIMUM, MAXIMUM * 4).unwrap();
        let mut operation = prepare(placement, &mut store).unwrap();
        let value = store.store(&input).unwrap();
        let mut inputs = [None; BROWSER_PORTS_PER_GEAR];
        inputs[0] = Some(value);
        let mut outputs = [None; BROWSER_PORTS_PER_GEAR];
        outputs[0] = Some(MAXIMUM);
        let mut io = StepIo::test_frame(inputs, [false; BROWSER_PORTS_PER_GEAR], outputs, None, 4);
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None; BROWSER_PORTS_PER_GEAR], None),
            ),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        input = codec.execute(OPERATIONS[index], &input).unwrap().to_vec();
        let output = store.store(&input).unwrap();
        let mut outputs = [None; BROWSER_PORTS_PER_GEAR];
        outputs[0] = Some(MAXIMUM);
        let mut io = StepIo::test_frame(
            [None; BROWSER_PORTS_PER_GEAR],
            [false; BROWSER_PORTS_PER_GEAR],
            outputs,
            Some((
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(output, MAXIMUM).unwrap()),
                    failure: None,
                },
            )),
            4,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None; BROWSER_PORTS_PER_GEAR], None),
            ),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), Some(output));
        let mut closed = [false; BROWSER_PORTS_PER_GEAR];
        closed[0] = true;
        let mut io = StepIo::test_frame(
            [None; BROWSER_PORTS_PER_GEAR],
            closed,
            [None; BROWSER_PORTS_PER_GEAR],
            None,
            4,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None; BROWSER_PORTS_PER_GEAR], None),
            ),
            StepOutcome::Complete
        );
    }
    assert_eq!(
        input,
        conduit_semantic_catalog::normalized_value(&[333_333, 1_000_000])
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
}

#[test]
fn prepared_timing_refuses_bad_sequences_bounds_and_changed_placement() {
    let placements = placements();
    let placement = placements
        .iter()
        .find(|p| p.implementation_id.as_str() == IMPLEMENTATIONS[0])
        .unwrap();
    let mut codec = PreparedTiming::for_placement(placement).unwrap().unwrap();
    let mut bytes =
        conduit_semantic_catalog::timed_event_sequence_value("fixture/clock", &[100, 200, 500])
            .unwrap()
            .canonical_bytes()
            .unwrap();
    let at = bytes.windows(11).position(|v| v == b"100,200,500").unwrap();
    bytes[at..at + 11].copy_from_slice(b"100,100,500");
    assert_eq!(codec.execute(OPERATIONS[0], &bytes), Err(failure(4)));
    let mut store = HostedValueStore::new(2, MAXIMUM, MAXIMUM * 2).unwrap();
    let mut operation = prepare(placement, &mut store).unwrap();
    let value = store.store(&bytes).unwrap();
    let mut inputs = [None; BROWSER_PORTS_PER_GEAR];
    inputs[0] = Some(value);
    let mut io = StepIo::test_frame(
        inputs,
        [false; BROWSER_PORTS_PER_GEAR],
        [None; BROWSER_PORTS_PER_GEAR],
        None,
        4,
    );
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([None; BROWSER_PORTS_PER_GEAR], None),
        ),
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame(
        [None; BROWSER_PORTS_PER_GEAR],
        [false; BROWSER_PORTS_PER_GEAR],
        [None; BROWSER_PORTS_PER_GEAR],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(codec.execute(OPERATIONS[0], &bytes).unwrap_err()),
            },
        )),
        4,
    );
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([None; BROWSER_PORTS_PER_GEAR], None),
        ),
        StepOutcome::Fail(failure(4))
    );

    assert_eq!(codec.execute(OPERATIONS[0], &[0]), Err(failure(1)));
    assert_eq!(
        codec.execute(OPERATIONS[0], &vec![0; MAXIMUM as usize + 1]),
        Err(failure(1))
    );
    assert_eq!(codec.execute(OPERATIONS[1], &bytes), Err(failure(1)));
    let mut changed = placement.clone();
    changed.host_calls[0].maximum_output_bytes -= 1;
    assert!(PreparedTiming::for_placement(&changed).is_err());
}
