//! Actual exclusive USB possession offered by its same-process Linux Root.
//!
//! Move the actual initialized `StdHost` into `UsbSourceRoot::acquire` with
//! independently checked Host configuration and qualified device firmware hash.
//! Run `sample().await` on the Root-owned capture worker, never a display loop:
//! the existing USB adapter performs synchronous, bounded native IO (31-45s).
//! Successful samples retain the same physical session across child Plays;
//! failures or cancellation retire it. Each new acquisition has a fresh session
//! identifier and collector sequence; no continuity across reacquisition is claimed.
//! This local execution has no remote Body membership and signs no MCU evidence.
use crate::device_carrier_source::DeviceEnvironmentalSource;
use conduit_core::*;
use conduit_environment_runtime::environmental_reading::source_observation::{
    ObservationContract, MAXIMUM_BYTES,
};
use conduit_environment_runtime::environmental_source::{
    EnvironmentalSourceHostCall, EnvironmentalSourceProvider, AUTHORITY_CONTRACT, HOST_CALL,
    RESOURCE_CLASS,
};
use conduit_host_make::CheckedHostConfiguration;
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
use std::{cell::RefCell, io::Read, path::Path, rc::Rc, time::Duration};
pub(super) type SharedHost = Rc<RefCell<Option<conduit_std_host::StdHost>>>;
pub const CAPABILITY: &str = "linux/environmental-source@1";
pub const PROFILE: &str = "linux/environmental-source-bounded@1";
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Acquisition,
    Possession,
    Advertisement,
    WrongBoot,
    Authority,
    Plan,
    Planning(Box<conduit_planner::PlannerError>),
    Capacity,
}
struct Identity {
    host_id: HostId,
    boot_id: BootId,
    base_id: HostBaseId,
    provider_instance_id: BaseInstanceId,
    provider_generation: u64,
    resource_pool_id: ResourcePoolId,
    resource_generation_id: ResourceGenerationId,
    envelope_id: CapabilityEnvelopeId,
    artifact_id: ArtifactId,
    implementation_id: ImplementationId,
    base_implementation_id: BaseImplementationId,
}
/// Owns the local Root advertisement and actual initialized peripheral together.
/// Never reconstructed from a received Plan, member receipt or device report.
pub struct UsbSourceRoot<P = DeviceEnvironmentalSource> {
    host: HostAdvertisement,
    installed: Option<SharedHost>,
    identity: Identity,
    provider: Option<P>,
    key: [u8; 32],
    next_play: u64,
}
impl UsbSourceRoot {
    pub fn acquire(
        installed: conduit_std_host::StdHost,
        path: &Path,
        configuration: &CheckedHostConfiguration,
        firmware: [u8; 32],
        budget: Duration,
    ) -> Result<Self, Refusal> {
        let host = installed.advertisement().clone();
        let provider =
            DeviceEnvironmentalSource::acquire(path, &host, configuration, firmware, budget)
                .map_err(|_| Refusal::Acquisition)?;
        Self::attach(Rc::new(RefCell::new(Some(installed))), provider, "usb")
    }
}
impl<P: EnvironmentalSourceProvider> UsbSourceRoot<P> {
    pub(super) fn attach(
        installed: SharedHost,
        provider: P,
        realization: &str,
    ) -> Result<Self, Refusal> {
        let mut host = installed
            .borrow()
            .as_ref()
            .ok_or(Refusal::Acquisition)?
            .advertisement()
            .clone();
        let mut key = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut key))
            .map_err(|_| Refusal::Acquisition)?;
        let mut entropy = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut entropy))
            .map_err(|_| Refusal::Acquisition)?;
        let nonce = entropy
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        let identity = Identity {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            base_id: format!("base/environment-source/{nonce}").into(),
            provider_instance_id: format!("instance/environment-source/{nonce}").into(),
            provider_generation: 1,
            resource_pool_id: format!("pool/environment-source/{nonce}").into(),
            resource_generation_id: ResourceGenerationId(format!(
                "generation/environment-source/{nonce}"
            )),
            envelope_id: format!("envelope/environment-source/{nonce}").into(),
            artifact_id: format!("native/linux/environment-source-{realization}@1").into(),
            implementation_id: format!("linux/environment-source-{realization}@1").into(),
            base_implementation_id: format!("linux/environment-source-{realization}-base@1").into(),
        };
        let mut root = Self {
            host: host.clone(),
            installed: Some(installed),
            identity,
            provider: Some(provider),
            key,
            next_play: 0,
        };
        root.append_to_advertisement(&mut host)?;
        host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        host.capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        root.host = host;
        Ok(root)
    }
}
impl<P: EnvironmentalSourceProvider> UsbSourceRoot<P> {
    pub fn host(&self) -> &HostAdvertisement {
        &self.host
    }
    /// Project only the actual retained provider into the matching Host/Boot.
    /// Preparation may allocate; this method starts no timer work.
    fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
    ) -> Result<(), Refusal> {
        use Refusal as R;
        let identity = &self.identity;
        if advertisement.host_id != identity.host_id || advertisement.boot_id != identity.boot_id {
            return Err(R::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == identity.base_id
                || base.provider_instance_id == identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|c| c.capability_id.as_str() == CAPABILITY)
            || advertisement
                .resources
                .iter()
                .any(|r| r.pool_id == identity.resource_pool_id)
        {
            return Err(R::Advertisement);
        }
        let kind = ObservationContract::prepare()
            .map_err(|_| R::Advertisement)?
            .kind(false)
            .map_err(|_| R::Advertisement)?;
        let offer = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: CAPABILITY.into(),
                execution_profile_id: PROFILE.into(),
                implementation_id: identity.implementation_id.clone(),
                artifact_id: identity.artifact_id.clone(),
                host_calls: vec![HostCallRequirement {
                    contract_id: HOST_CALL.into(),
                    target_kind: Some(kind.kind_id.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: 1,
                    maximum_output_bytes: MAXIMUM_BYTES as u32,
                }],
                resource_requirements: vec![resource_requirement(RESOURCE_CLASS, 1)],
                authority_requirements: vec![AuthorityRequirement {
                    contract_id: AUTHORITY_CONTRACT.into(),
                    host_call_contract_id: HOST_CALL.into(),
                    subject_kind: kind.kind_id.clone(),
                }],
            },
        )
        .build();
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                advertisement.capabilities.len().saturating_add(1),
            )
            .map_err(|_| R::Advertisement)?,
            maximum_advertised_resources: u16::try_from(
                advertisement.resources.len().saturating_add(1),
            )
            .map_err(|_| R::Advertisement)?,
        })
        .map_err(|_| R::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: identity.base_id.clone(),
                provider_instance_id: identity.provider_instance_id.clone(),
                provider_generation: identity.provider_generation,
                implementation_id: identity.base_implementation_id.clone(),
                mechanism_family: RESOURCE_CLASS.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    identity.resource_pool_id.as_str(),
                    RESOURCE_CLASS,
                    1,
                )],
            })
            .map_err(|_| R::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| R::Advertisement)
    }
}

mod planning;
mod runner;
pub use runner::CommittedSample;
#[cfg(test)]
mod tests;

impl<P> Drop for UsbSourceRoot<P> {
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

mod composite;
mod network;
pub use composite::{CollectorRoot, CollectorSourceId};
pub use network::NetworkSourceConfiguration;
