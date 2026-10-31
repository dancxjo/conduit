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
pub(super) struct Identity {
    host_id: HostId,
    boot_id: BootId,
    base_id: HostBaseId,
    capability: CapabilityId,
    publication: CapabilityId,
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
    installed: SharedHost,
    identity: Identity,
    provider: Option<P>,
    key: [u8; 32],
    next_play: u64,
    failed: bool,
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
        Self::attach_with_inventory(installed, provider, realization, |_, _, _| Ok(()))
    }
    pub(super) fn attach_with_inventory(
        installed: SharedHost,
        provider: P,
        realization: &str,
        configure: impl FnOnce(&mut P, &Identity, &mut BaseProviderEntry) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        let host = installed
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
            capability: format!("environment/source/{nonce}").into(),
            publication: format!("environment/publication/{nonce}").into(),
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
            installed,
            identity,
            provider: Some(provider),
            key,
            next_play: 0,
            failed: false,
        };
        let mut base = root.base_entry()?;
        configure(
            root.provider
                .as_mut()
                .expect("retained initialized provider"),
            &root.identity,
            &mut base,
        )?;
        // The actual initialized provider remains retained in root throughout
        // this idle, transactional installation on the one installed Host.
        unsafe {
            root.installed
                .borrow_mut()
                .as_mut()
                .ok_or(Refusal::Acquisition)?
                .install_retained_native_base(base)
        }
        .map_err(|_| Refusal::Advertisement)?;
        Ok(root)
    }
    pub fn advertisement(&self) -> Result<HostAdvertisement, Refusal> {
        Ok(self
            .installed
            .borrow()
            .as_ref()
            .ok_or(Refusal::Acquisition)?
            .advertisement()
            .clone())
    }
    fn base_entry(&self) -> Result<BaseProviderEntry, Refusal> {
        let identity = &self.identity;
        type R = Refusal;
        let kind = ObservationContract::prepare()
            .map_err(|_| R::Advertisement)?
            .kind(false)
            .map_err(|_| R::Advertisement)?;
        let offer = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: identity.capability.clone(),
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
        let publication_kind = ObservationContract::prepare()
            .map_err(|_| R::Advertisement)?
            .kind(true)
            .map_err(|_| R::Advertisement)?;
        let publication = BackOfferBuilder::new(
            publication_kind,
            Back {
                capability_id: identity.publication.clone(),
                execution_profile_id: PROFILE.into(),
                implementation_id: "linux/environmental-publication@1".into(),
                artifact_id: "native/environmental-publication@1".into(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build();
        Ok(BaseProviderEntry {
            base_id: identity.base_id.clone(),
            provider_instance_id: identity.provider_instance_id.clone(),
            provider_generation: identity.provider_generation,
            implementation_id: identity.base_implementation_id.clone(),
            mechanism_family: RESOURCE_CLASS.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer, publication],
            resources: vec![resource_offer(
                identity.resource_pool_id.as_str(),
                RESOURCE_CLASS,
                1,
            )],
        })
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
pub use composite::{
    CollectorRoot, CollectorSourceId, DetachRefusal, DetachedCollectorEquipment, ResumeRefusal,
};
pub use network::NetworkSourceConfiguration;
