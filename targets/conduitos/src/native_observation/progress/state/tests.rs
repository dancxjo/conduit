use super::*;
#[test]
fn nested_scope_records_have_exact_order_parent_clock_and_outcome() {
    let state = Tracker::new();
    let (outer, first) = state
        .begin(Phase::PlotBuild, "native/plot", 10)
        .unwrap()
        .unwrap();
    let (inner, second) = state
        .begin(Phase::SourceCheck, "native/plot", 20)
        .unwrap()
        .unwrap();
    let third = state.close(inner, Event::Refused, 30).unwrap().unwrap();
    let fourth = state.close(outer, Event::Finished, 40).unwrap().unwrap();
    for (index, record) in [first, second, third, fourth].iter().enumerate() {
        assert_eq!(record.sequence, index as u32);
        let mut line = Line::new();
        record.write(&mut line).unwrap();
        let text = core::str::from_utf8(line.bytes()).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(text.strip_prefix("CONDUIT_NATIVE_PROGRESS ").unwrap()).unwrap();
        assert_eq!(value["span"], if index == 1 || index == 2 { 2 } else { 1 });
        assert_eq!(
            value["parent"],
            if index == 1 || index == 2 { 1 } else { 0 }
        );
        assert_eq!(value["ticks"], 10 * (index + 1));
        assert!(value["frequency_hz"].is_null());
        assert_eq!(
            value["state"],
            ["started", "started", "refused", "finished"][index]
        );
    }
}
#[test]
fn scope_or_clock_corruption_freezes_diagnostics_after_one_explicit_refusal() {
    for reverse_clock in [false, true] {
        let state = Tracker::new();
        let (outer, _) = state
            .begin(Phase::FaceConstruction, "product", 10)
            .unwrap()
            .unwrap();
        let token = if reverse_clock {
            outer
        } else {
            state.begin(Phase::SourceCheck, "product", 20).unwrap();
            outer
        };
        assert!(
            state
                .close(token, Event::Finished, if reverse_clock { 9 } else { 30 })
                .is_err()
        );
        assert!(
            state
                .begin(Phase::FaceConstruction, "product", 40)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn exact_finite_span_budget_refuses_without_flooding_or_wrapping() {
    let state = Tracker::new();
    for id in 1..=MAX_SPANS {
        let (token, start) = state
            .begin(Phase::Scanout, "frame", u64::from(id) * 2)
            .unwrap()
            .unwrap();
        assert_eq!(start.span, id);
        let end = state
            .close(token, Event::Finished, u64::from(id) * 2 + 1)
            .unwrap()
            .unwrap();
        assert_eq!(end.sequence, id * 2 - 1);
    }
    assert!(matches!(
        state.begin(Phase::Scanout, "frame", 9999),
        Err("span-bound")
    ));
    for _ in 0..10 {
        assert!(
            state
                .begin(Phase::Scanout, "frame", 9999)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn bounded_context_cannot_inject_json_and_maximum_record_fits_fixed_storage() {
    for context in ["", "quoted\"context", "line\ncontext"] {
        assert!(Tracker::new().begin(Phase::PlotBuild, context, 0).is_err());
    }
    let state = Tracker::new();
    let (_, record) = state
        .begin(
            Phase::IdentityGeneration,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            u64::MAX,
        )
        .unwrap()
        .unwrap();
    let mut line = Line::new();
    record.write(&mut line).unwrap();
    assert!(line.bytes().len() <= 512);
    let value: serde_json::Value =
        serde_json::from_slice(&line.bytes()["CONDUIT_NATIVE_PROGRESS ".len()..]).unwrap();
    assert_eq!(value["ticks"], u64::MAX);
}
#[test]
fn an_abandoned_scope_is_not_reported_as_a_success_or_operation_refusal() {
    let state = Tracker::new();
    let (token, _) = state
        .begin(Phase::SourceExpand, "source", 1)
        .unwrap()
        .unwrap();
    let record = state.close(token, Event::Abandoned, 2).unwrap().unwrap();
    assert_eq!(record.event, Event::Abandoned);
}
