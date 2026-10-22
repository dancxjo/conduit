//! Admitted native implementations for the portable application Plot seam.

use alloc::{format, vec};
use conduit_core::{CapabilityOffer, HostCallRequirement, resource_requirement};
pub use conduitos_application_core::data::Proposal as NativeApplicationRequest;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub(super) type NativeApplications = crate::protected_application::Owner;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub(super) type NativeApplications = conduitos_application_core::models::Models;

pub(super) fn initial(
    prepared: &super::PreparedNativeWorkset,
    evidence: Option<&conduit_body::BodyBiographyEvidence>,
) -> Result<
    [Option<conduitos_application_core::data::Initial>; super::NATIVE_PLOT_CAPACITY],
    super::WorksetRefusal,
> {
    use conduitos_application_core::data::{Initial, Target};
    let mut initial = core::array::from_fn(|_| None);
    for (slot, planned) in prepared.plan.plots.iter().enumerate() {
        let Some(placement) = planned.plan.fragments[0].placements.iter().find(|p| {
            p.implementation_id.as_str() == STATE_IMPLEMENTATION
                || p.implementation_id.as_str() == super::patchbay_ingress::IMPLEMENTATION
        }) else {
            continue;
        };
        let application =
            if placement.implementation_id.as_str() == super::patchbay_ingress::IMPLEMENTATION {
                "patchbay"
            } else {
                super::tutorial_application::selected_application(placement)?
            };
        initial[slot] = Some(match application {
            "tutorial" => Initial::Tutorial {
                evidence: evidence.ok_or(super::WorksetRefusal::Plan)?.clone(),
            },
            "tour" => Initial::Tour,
            "patchbay" => {
                let mut targets = core::array::from_fn(|_| None);
                for (index, planned) in prepared.plan.plots.iter().enumerate() {
                    if index == slot {
                        continue;
                    }
                    let expanded = super::checked(super::resolve(&planned.plot)?)?;
                    let graph = patchbay_graph::PatchbayGraph::from_expanded(&expanded)
                        .map_err(|_| super::WorksetRefusal::Plan)?;
                    targets[index] = Some(Target {
                        graph,
                        plan: planned.plan.plan_id.clone(),
                        body: prepared.plan.plan_id.clone(),
                    });
                }
                Initial::Patchbay {
                    targets,
                    selected: if slot == 0 { 1 } else { 0 },
                }
            }
            _ => return Err(super::WorksetRefusal::Plan),
        });
    }
    Ok(initial)
}

pub(super) const EVENT_IMPLEMENTATION: &str = "conduitos/application-event-delivery@1";
pub(super) const STATE_IMPLEMENTATION: &str = "conduitos/retained-application@1";
pub(super) const FACE_IMPLEMENTATION: &str = "conduitos/application-view-face@1";
pub(super) const EVENT_OPERATION: &str = "conduit.host/application-next-event@1";
pub(super) const STATE_OPERATION: &str = "conduit.host/application-apply-event@1";
pub(super) const FACE_OPERATION: &str = "conduit.host/present-application-view@1";
pub(super) const EVENT_BYTES: u32 = 128;
pub(super) const VIEW_BYTES: u32 = 3 * 1024;

pub(super) fn offers(build: &str) -> [CapabilityOffer; 3] {
    [
        offer(
            conduit_semantic_catalog::event_source_contract(),
            EVENT_IMPLEMENTATION,
            EVENT_OPERATION,
            0,
            EVENT_BYTES,
            build,
        ),
        offer(
            conduit_semantic_catalog::retained_application_contract(),
            STATE_IMPLEMENTATION,
            STATE_OPERATION,
            EVENT_BYTES,
            VIEW_BYTES,
            build,
        ),
        offer(
            conduit_semantic_catalog::view_face_contract(),
            FACE_IMPLEMENTATION,
            FACE_OPERATION,
            VIEW_BYTES,
            0,
            build,
        ),
    ]
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    implementation: &'static str,
    operation: &'static str,
    input: u32,
    output: u32,
    build: &str,
) -> CapabilityOffer {
    let mut offer = conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::APPLICATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: "conduitos/bounded-application@1",
            implementation,
            artifact: "conduitos/application@1",
        },
        vec![HostCallRequirement {
            contract_id: operation.into(),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: input,
            maximum_output_bytes: output,
        }],
        vec![resource_requirement(
            "conduit.resource/runtime-memory@1",
            VIEW_BYTES,
        )],
        vec![],
    );
    offer.implementation.artifact_id = format!("conduitos-build/{build}").into();
    offer.limits.max_queue_items = 1;
    offer.limits.max_queue_bytes = input.max(output);
    offer
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{
        ApplicationComponent, ApplicationEvent, ApplicationEventKind, ApplicationView,
    };

    #[test]
    fn every_native_patchbay_inspection_fits_the_admitted_view_value() {
        let (ids, offer) = super::super::tests::fixture();
        let wake = super::super::tests::wake(&super::super::inventory());
        let prepared = super::super::prepare(&wake, &ids, &offer, "build").unwrap();
        let resident = super::super::resident(super::super::NativePlot::Patchbay).unwrap();
        let index = prepared
            .plan
            .plots
            .iter()
            .position(|plot| plot.plot == resident)
            .unwrap();
        let born = conduit_body::Body::born_with_plots(wake.workset.clone(), 1, "sign/born".into())
            .unwrap();
        let membership = conduit_body::BodyMembership::new(born.body_id.clone()).unwrap();
        let evidence =
            conduit_body::BodyBiographyEvidence::born(born, membership, "Test Body".into())
                .unwrap();
        let initial = initial(&prepared, Some(&evidence)).unwrap();
        let mut targets = conduitos_application_core::models::Models::new(initial).unwrap();
        for target in 0..super::super::NATIVE_PLOT_CAPACITY {
            if target == index {
                continue;
            }
            targets.select(index, target).unwrap();
            let count = targets.graph(index).unwrap().subject_identities().count();
            for _ in 0..count {
                let (output, _) = targets.apply(index, &[]).unwrap();
                assert!(
                    output.view.len() <= VIEW_BYTES as usize,
                    "native inspector emitted {} bytes beyond admitted {}",
                    output.view.len(),
                    VIEW_BYTES
                );
                let view = ApplicationView::decode(&output.view).unwrap();
                assert!(view.nodes.iter().any(|node| node.key == "subject"));
                assert!(view.nodes.iter().any(|node| node.key == "exact-evidence"));
                assert!(
                    !view
                        .nodes
                        .iter()
                        .any(|node| node.component == ApplicationComponent::PatchbayCanvas)
                );
                let event = ApplicationEvent {
                    revision: view.revision,
                    action: patchbay_application::INSPECT_NEXT_ACTION_ID.into(),
                    kind: ApplicationEventKind::Activate,
                    value: alloc::vec::Vec::new(),
                };
                targets.apply(index, &event.encode(&view).unwrap()).unwrap();
            }
        }
    }
}
