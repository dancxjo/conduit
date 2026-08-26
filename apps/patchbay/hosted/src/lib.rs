//! Concrete std-Host adapter for Patchbay application compositions.

use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration, Plan, PlanFragment};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use patchbay_model::{
    ControlReceiptProjection, PatchbayHostAdapter, PatchbayHostExecution, PatchbayHostProfile,
    PlayExecutionProjection,
};

#[derive(Debug, Default)]
pub struct HostedPatchbayAdapter;

impl PatchbayHostAdapter for HostedPatchbayAdapter {
    fn advertisement(
        &self,
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: PatchbayHostProfile,
    ) -> Result<HostAdvertisement, String> {
        let composition = match profile {
            PatchbayHostProfile::Signal => StdHostComposition::minimal().with_signal(),
            PatchbayHostProfile::Text => StdHostComposition::minimal().with_text(),
            PatchbayHostProfile::Reference => StdHostComposition::reference(),
            PatchbayHostProfile::PicoSimulation => StdHostComposition::minimal()
                .with_signal()
                .with_time()
                .with_state()
                .with_logic()
                .with_math()
                .with_robotics(),
        };
        Ok(StdHost::new_with_composition(
            StdHostConfig {
                host_id,
                boot_id,
                offer_generation,
            },
            composition,
        )
        .advertisement()
        .clone())
    }

    fn plan_expanded_local(
        &self,
        advertisement: &HostAdvertisement,
        expanded: &conduit_form::ExpandedCanonicalForm,
    ) -> Result<Plan, String> {
        StdHost::from_advertisement(advertisement.clone())?
            .plan_expanded_local(expanded)
            .map_err(|error| error.to_string())
    }

    fn run_fragment(
        &self,
        advertisement: &HostAdvertisement,
        fragment: PlanFragment,
    ) -> Result<PatchbayHostExecution, String> {
        let mut host = StdHost::from_advertisement(advertisement.clone())?;
        let mut output = Vec::with_capacity(4096);
        let report = host.run_fragment_to(fragment, &mut output, &mut ThreadTimer)?;
        let kernel = report
            .kernel
            .as_ref()
            .ok_or("std Host omitted its kernel execution report")?;
        Ok(PatchbayHostExecution {
            projection: PlayExecutionProjection {
                active_play_id: kernel.active_play_id.clone(),
                decisions: kernel.decisions,
                kernel_events: kernel.kernel_events,
                kernel_sign: kernel.kernel_sign.clone(),
                observations: report.observations,
                control_receipts: report
                    .control_receipts
                    .into_iter()
                    .map(|receipt| ControlReceiptProjection {
                        request_id: receipt.request_id.as_str().into(),
                        disposition: format!("{:?}", receipt.disposition),
                        active_play_id: receipt.active_play_id,
                    })
                    .collect(),
            },
            output,
        })
    }
}
