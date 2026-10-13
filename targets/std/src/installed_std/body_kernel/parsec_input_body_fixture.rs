//! Shared actual Host/Body preparation for the retained combined-input proofs.
use super::*;

pub(in crate::installed_std::body_kernel) fn prepare() -> (
    StdHost,
    RunControl,
    BodyLiveForeQueue,
    conduit_body::Wake,
    BodyPlan,
) {
    let baseline = BodyLiveForeQueue::parsec_input_test_host(StdHostConfig {
        host_id: "std/combined-map-input-proof".into(),
        boot_id: "std/combined-map-input-proof/boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut advertisement = baseline.advertisement().clone();
    let offers = crate::terminal_mask_execution::planning::attached_terminal_capabilities()
        .into_iter()
        .chain(
            crate::terminal_mask_execution::planning::parsec_instruction_capabilities().unwrap(),
        );
    for offer in offers {
        if let Some(existing) = advertisement
            .capabilities
            .iter()
            .find(|old| old.capability_id == offer.capability_id)
        {
            assert_eq!(existing, &offer);
        } else {
            advertisement.capabilities.push(offer);
        }
    }
    let resources = crate::terminal_mask_execution::planning::attached_terminal_resources()
        .into_iter()
        .chain(crate::terminal_mask_execution::planning::parsec_instruction_resources());
    for resource in resources {
        if !advertisement.resources.contains(&resource) {
            advertisement.resources.push(resource);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement.resources.sort();
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let selected = BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_parsec_plan(&selected, control.clone(), 1).unwrap();
    let body = Body::born(
        selected.source_document_id.clone(),
        selected.checked_plot_id.clone(),
        1,
        SignId::from("sign/combined-map-input-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/combined-map-input-wake"))
        .unwrap()
        .1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                selected.source_document_id.clone(),
                selected.checked_plot_id.clone(),
            ),
            plan: selected,
        }],
    )
    .unwrap();
    (host, control, queue, wake, plan)
}
