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
        let output = match execution.execute(quantity) {
            Ok(output) => output,
            Err(why) => {
                let report = execution.finish().unwrap();
                panic!(
                    "Value {index}: {why}; terminal {:?}; failure {:?}; children {:?}",
                    report.terminal, report.failure, report.each_child_signs
                );
            }
        };
        assert_eq!(output.value.source(), input);
        assert_eq!(output.play, play);
        assert!(!output.canonical.is_empty());
    }
    assert!(execution.execute(Quantity::new(65, Unit::Meter)).is_err());
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

#[test]
fn malformed_input_does_not_spend_a_value_and_stop_retains_accepted_evidence() {
    let (host, plan) = prepared();
    let mut execution = NumericalFore::from_plan(host, plan).unwrap();
    assert!(execution.execute_encoded(&[0; 3]).is_err());
    execution.execute(Quantity::new(3, Unit::Meter)).unwrap();
    let report = execution.stop().unwrap();
    assert_ne!(report.terminal, TerminalDisposition::Completed);
    let receipts = report.each_child_signs.unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].invocation, 0);
    assert!(!receipts[0].remote_lifecycle.is_empty());
    assert!(!report.each_cancellation_failed);
}

#[test]
fn producer_pressure_and_close_do_not_admit_an_extra_value() {
    use conduit_std_host::{BodyLiveForeAdmission, BodyLiveForeQueue, RunControl};
    let (_, plan) = prepared();
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_near_each_plan(&plan, control, 1).unwrap();
    let bytes = Quantity::new(3, Unit::Meter).encode();
    assert_eq!(
        queue.submit(&bytes).unwrap(),
        BodyLiveForeAdmission::Accepted { sequence: 0 }
    );
    assert_eq!(queue.submit(&bytes).unwrap(), BodyLiveForeAdmission::Full);
    queue.close().unwrap();
    assert!(queue.submit(&bytes).is_err());
}

#[test]
fn substituted_plan_and_replacement_host_cannot_start_the_numerical_owner() {
    let (host, mut plan) = prepared();
    plan.plan_id = "foreign-plan".into();
    assert!(NumericalFore::from_plan(host, plan).is_err());
    let (_, plan) = prepared();
    let replacement = StdHost::new_with_composition(
        StdHostConfig {
            host_id: plan.fragments[0].host_id.clone(),
            boot_id: "replacement-boot".into(),
            offer_generation: plan.fragments[0].offer_generation,
        },
        StdHostComposition::minimal(),
    );
    assert!(NumericalFore::from_plan(replacement, plan).is_err());
}
