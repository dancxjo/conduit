//! Admitted native implementations for the portable application Form seam.

use alloc::{boxed::Box, format, vec};
use conduit_core::{CapabilityOffer, HostOperationRequirement, resource_requirement};

pub(super) enum NativeApplication {
    Tour(Box<conduit_tour_model::TourApplicationPort>),
    Patchbay(PatchbayTargets),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeApplicationRequest {
    RunTour,
    OpenPatchbay,
    EditCurrent(patchbay_application::PatchbayApplicationRequest),
}

pub(super) struct PatchbayTargets {
    ports:
        [Option<Box<patchbay_application::PatchbayApplicationPort>>; super::NATIVE_FORM_CAPACITY],
    selected: usize,
}

impl PatchbayTargets {
    pub(super) fn prepare(
        prepared: &super::PreparedNativeWorkset,
        patchbay: usize,
    ) -> Result<Self, super::WorksetRefusal> {
        let mut selected = 0;
        if selected == patchbay {
            selected = 1;
        }
        let mut ports = core::array::from_fn(|_| None);
        for (index, planned) in prepared.plan.forms.iter().enumerate() {
            if index == patchbay {
                continue;
            }
            let native = super::resolve(&planned.form)?;
            let expanded = super::checked(native)?;
            ports[index] = Some(Box::new(
                patchbay_application::PatchbayApplicationPort::open(
                    &expanded,
                    planned.plan.plan_id.clone(),
                    prepared.plan.plan_id.clone(),
                )
                .map_err(|_| super::WorksetRefusal::Plan)?,
            ));
        }
        Ok(Self { ports, selected })
    }

    pub(super) fn select(&mut self, target: usize) -> Result<(), super::play::PlayRefusal> {
        if self.ports.get(target).is_none_or(Option::is_none) {
            return Err(super::play::PlayRefusal::Foreground);
        }
        self.selected = target;
        Ok(())
    }

    pub(super) fn apply(
        &mut self,
        input: &[u8],
    ) -> Result<patchbay_application::PatchbayApplicationOutput, super::play::PlayRefusal> {
        self.ports[self.selected]
            .as_mut()
            .ok_or(super::play::PlayRefusal::Kernel)?
            .apply(input)
            .map_err(|_| super::play::PlayRefusal::Kernel)
    }

    pub(super) fn set_presenter_topology(
        &mut self,
        topology: &patchbay_application::PatchbayPresenterTopology,
    ) {
        for port in self.ports.iter_mut().flatten() {
            port.set_presenter_topology(topology.clone());
        }
    }
}

pub(super) const EVENT_IMPLEMENTATION: &str = "conduitos/application-event-delivery@1";
pub(super) const STATE_IMPLEMENTATION: &str = "conduitos/retained-application@1";
pub(super) const PRESENTATION_IMPLEMENTATION: &str = "conduitos/application-view-presentation@1";
pub(super) const EVENT_OPERATION: &str = "conduit.host/application-next-event@1";
pub(super) const STATE_OPERATION: &str = "conduit.host/application-apply-event@1";
pub(super) const PRESENTATION_OPERATION: &str = "conduit.host/present-application-view@1";
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
            conduit_semantic_catalog::view_presentation_contract(),
            PRESENTATION_IMPLEMENTATION,
            PRESENTATION_OPERATION,
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
        vec![HostOperationRequirement {
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
