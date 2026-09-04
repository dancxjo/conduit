//! Application-edge contract for concrete hosted construction and execution.

use crate::PlayExecutionProjection;
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration, Plan, PlanFragment};
use conduit_form::ExpandedCanonicalForm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbayHostProfile {
    Signal,
    Text,
    Reference,
    PicoSimulation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayHostExecution {
    pub projection: PlayExecutionProjection,
    pub output: Vec<u8>,
}

pub trait PatchbayHostAdapter: Send + Sync {
    fn advertisement(
        &self,
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: PatchbayHostProfile,
    ) -> Result<HostAdvertisement, String>;

    fn plan_expanded_local(
        &self,
        advertisement: &HostAdvertisement,
        expanded: &ExpandedCanonicalForm,
    ) -> Result<Plan, String>;

    fn run_fragment(
        &self,
        advertisement: &HostAdvertisement,
        fragment: PlanFragment,
    ) -> Result<PatchbayHostExecution, String>;
}

#[cfg(test)]
pub(crate) fn test_host_adapter() -> &'static dyn PatchbayHostAdapter {
    &TestHostAdapter
}

#[cfg(test)]
pub(crate) fn test_host_adapter_arc() -> std::sync::Arc<dyn PatchbayHostAdapter> {
    std::sync::Arc::new(TestHostAdapter)
}

#[cfg(test)]
struct TestHostAdapter;

#[cfg(test)]
impl PatchbayHostAdapter for TestHostAdapter {
    fn advertisement(
        &self,
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: PatchbayHostProfile,
    ) -> Result<HostAdvertisement, String> {
        use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
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
        expanded: &ExpandedCanonicalForm,
    ) -> Result<Plan, String> {
        conduit_std_host::StdHost::from_advertisement(advertisement.clone())?
            .plan_expanded_local(expanded)
            .map_err(|error| error.to_string())
    }

    fn run_fragment(
        &self,
        advertisement: &HostAdvertisement,
        fragment: PlanFragment,
    ) -> Result<PatchbayHostExecution, String> {
        use conduit_std_host::{StdHost, ThreadTimer};
        let mut host = StdHost::from_advertisement(advertisement.clone())?;
        let mut output = Vec::with_capacity(4096);
        let report = host.run_fragment_to(fragment, &mut output, &mut ThreadTimer)?;
        let kernel = report.kernel.ok_or("test Host omitted its kernel report")?;
        Ok(PatchbayHostExecution {
            projection: PlayExecutionProjection {
                active_play_id: kernel.active_play_id,
                decisions: kernel.decisions,
                kernel_events: kernel.kernel_events,
                kernel_sign: kernel.kernel_sign,
                observations: report.observations,
                control_receipts: report
                    .control_receipts
                    .into_iter()
                    .map(|receipt| crate::ControlReceiptProjection {
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
