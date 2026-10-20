use super::*;

fn prepared() -> (StdHost, Plan) {
    let source = crate::plot_source::parse(include_str!(
        "../../../../plots/quantity-projection/main.conduit"
    ))
    .unwrap();
    let checked = source.check().unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "quantity-projection/main",
        source.authoring_catalog(),
    )
    .unwrap();
    prepare(&source, &checked, &authored).unwrap()
}

#[test]
fn authored_numerical_fore_retains_all_admitted_values_and_exact_child_evidence() {
    let (host, plan) = prepared();
    let PlannedActivationEntry::Unary(each) = &plan.activations[0] else {
        panic!("authored numerical Each must remain unary")
    };
    let selected = each.selected_plan_id.clone();
    let mut execution = NumericalFore::from_plan(host, plan).unwrap();
    let play = execution.play().clone();
    for index in 0..64 {
        let literal = format!("{}m", index + 1);
        let quantity = Quantity::parse_plot_literal(&literal).unwrap();
        let input =
            RationalQuantity::from_quantity(quantity, &mut ExactNumericWork::new(200_000_000))
                .unwrap();
        let output = execution.execute(quantity).unwrap();
        assert_eq!(output.value.source(), input);
        assert_eq!(output.play, play);
        assert!(!output.canonical.is_empty());
    }
    let report = execution.finish().unwrap();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(!report.each_cancellation_failed);
    assert!(!report.each_output_completion_failed);
    let receipts = report.each_child_signs.unwrap();
    assert_eq!(receipts.len(), 64);
    for (index, receipt) in receipts.iter().enumerate() {
        assert_eq!(receipt.selected_plan_id, selected);
        assert_eq!(receipt.invocation as usize, index);
        assert!(!receipt.events.is_empty());
        assert!(!receipt.remote_lifecycle.is_empty());
        assert!(receipts[..index]
            .iter()
            .all(|old| old.child_active_play_id != receipt.child_active_play_id));
    }
}

#[test]
fn empty_numerical_fore_does_not_report_unexecuted_reservations() {
    let (host, plan) = prepared();
    let report = NumericalFore::from_plan(host, plan)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.each_child_signs.unwrap().is_empty());
}
