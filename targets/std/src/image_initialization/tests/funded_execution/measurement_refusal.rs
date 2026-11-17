//! A reference offer is not an original IMAGE execution grant.
use super::*;

#[test]
fn registered_measurement_reference_back_cannot_bypass_selected_image_admission() {
    let (mut selected, _, _, mut clock) = fixture(4, 64 * 1024 * 1024);
    let ad = selected.advertisement();
    let reference = StdHost::new_with_config(crate::StdHostConfig {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
    });
    let measurement =
        crate::installed_std::measurement_plan_fixture(reference.advertisement()).unwrap();
    let resident = ResidentPlot::new(
        measurement.source_document_id.clone(),
        measurement.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/measurement-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/measurement-woke".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan: measurement,
        }],
    )
    .unwrap();
    let counter = selected.kernel_resources.image_memory_observer();
    let baseline = counter.load(Ordering::Acquire);
    let sequence = selected.next_kernel_play_sequence;
    let advertisement = selected.advertisement().clone();
    let control = crate::RunControl::default();
    let mut started = false;
    let mut text = Vec::new();
    let result = selected.run_selected_image_body_plan_to_with_start(
        request(&wake, &plan, &control),
        &mut text,
        &mut clock,
        |_, _| {
            started = true;
            Ok(())
        },
    );
    assert!(
        result.is_err(),
        "reference registration must not admit an unfunded IMAGE Back"
    );
    assert!(!started);
    assert_eq!(selected.next_kernel_play_sequence, sequence);
    assert_eq!(counter.load(Ordering::Acquire), baseline);
    assert_eq!(selected.advertisement(), &advertisement);
    assert!(selected.kernel_resources.is_idle());
    assert_eq!(clock.sampled, 0);
    assert!(text.is_empty());

    // The older ordinary Fore entrance must remain closed for the same IMAGE.
    let mut output = Output::new(0);
    let result = selected.run_body_plan_with_fore_to(
        request(&wake, &plan, &control),
        &[],
        false,
        &mut output,
        &mut text,
        &mut clock,
    );
    assert!(result.is_err());
    assert!(output.retained.is_empty());
    assert_eq!(selected.next_kernel_play_sequence, sequence);
    assert_eq!(counter.load(Ordering::Acquire), baseline);
    assert!(selected.kernel_resources.is_idle());
}
