//! Four finite source slots sharing one actual installed collector Host owner.
use super::network::NetworkSource;
use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::SourceObservation;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectorSourceId(usize);
enum Provider {
    Usb(DeviceEnvironmentalSource),
    Network(NetworkSource),
}
impl EnvironmentalSourceProvider for Provider {
    fn prepare_for_host(
        &mut self,
        host: &HostAdvertisement,
    ) -> Result<(), conduit_environment_runtime::environmental_source::Refusal> {
        match self {
            Self::Usb(p) => p.prepare_for_host(host),
            Self::Network(p) => p.prepare_for_host(host),
        }
    }

    async fn observe(
        &mut self,
    ) -> Result<SourceObservation, conduit_environment_runtime::environmental_source::Refusal> {
        match self {
            Self::Usb(p) => p.observe().await,
            Self::Network(p) => p.observe().await,
        }
    }
}
pub struct CollectorRoot {
    installed: SharedHost,
    slots: Vec<UsbSourceRoot<Provider>>,
    sealed: bool,
}
impl CollectorRoot {
    pub fn new(installed: conduit_std_host::StdHost) -> Self {
        Self {
            installed: Arc::new(Mutex::new(Some(installed))),
            slots: Vec::with_capacity(4),
            sealed: false,
        }
    }
    pub fn attach_usb(
        &mut self,
        path: &Path,
        configuration: &CheckedHostConfiguration,
        firmware: [u8; 32],
        budget: Duration,
    ) -> Result<CollectorSourceId, Refusal> {
        self.capacity()?;
        let provider = {
            let owner = self
                .installed
                .lock()
                .expect("collector Host mutex poisoned");
            let host = owner.as_ref().ok_or(Refusal::Acquisition)?;
            DeviceEnvironmentalSource::acquire(
                path,
                host.advertisement(),
                configuration,
                firmware,
                budget,
            )
            .map_err(|_| Refusal::Acquisition)?
        };
        self.attach(Provider::Usb(provider))
    }
    pub fn attach_network(
        &mut self,
        network: NetworkSourceConfiguration,
        configuration: &CheckedHostConfiguration,
        budget: Duration,
    ) -> Result<CollectorSourceId, Refusal> {
        self.capacity()?;
        let provider =
            NetworkSource::acquire(self.installed.clone(), network, configuration, budget)?;
        self.attach(Provider::Network(provider))
    }
    fn capacity(&self) -> Result<(), Refusal> {
        if self.sealed {
            return Err(Refusal::Authority);
        }
        if self.slots.len() >= 4 {
            Err(Refusal::Capacity)
        } else {
            Ok(())
        }
    }
    fn attach(&mut self, provider: Provider) -> Result<CollectorSourceId, Refusal> {
        let realization = match &provider {
            Provider::Usb(_) => "usb",
            Provider::Network(_) => "isolated-http",
        };
        let root = UsbSourceRoot::attach_with_inventory(
            self.installed.clone(),
            provider,
            realization,
            |provider, identity, base| match provider {
                Provider::Usb(_) => Ok(()),
                Provider::Network(network) => network.register_native(identity, base),
            },
        )?;
        let id = CollectorSourceId(self.slots.len());
        self.slots.push(root);
        Ok(id)
    }
    pub fn advertisement(&self) -> Result<HostAdvertisement, Refusal> {
        Ok(self
            .installed
            .lock()
            .expect("collector Host mutex poisoned")
            .as_ref()
            .ok_or(Refusal::Acquisition)?
            .advertisement()
            .clone())
    }
    /// Finalize all actual native slots before Boot publication. Later attachment
    /// refuses without IO; capture never changes the published inventory.
    pub fn seal_inventory(&mut self) -> Result<HostAdvertisement, Refusal> {
        let host = self.advertisement()?;
        if !self.sealed {
            for slot in &mut self.slots {
                slot.provider
                    .as_mut()
                    .ok_or(Refusal::Possession)?
                    .prepare_for_host(&host)
                    .map_err(|_| Refusal::Acquisition)?;
            }
            self.sealed = true;
        }
        Ok(host)
    }
    /// Serial capture is enforced by exclusive access to the same composite owner.
    pub async fn sample(&mut self, id: CollectorSourceId) -> Result<CommittedSample, Refusal> {
        self.seal_inventory()?;
        self.slots
            .get_mut(id.0)
            .ok_or(Refusal::Possession)?
            .sample()
            .await
    }
}

/// Affine retained native equipment while its actual Host is owned elsewhere.
/// This object exposes no acquisition or sampling API. Its shared Host cell is empty.
pub struct DetachedCollectorEquipment {
    collector: CollectorRoot,
    inventory: HostAdvertisement,
}
pub struct DetachRefusal {
    pub collector: CollectorRoot,
    pub reason: Refusal,
}
pub struct ResumeRefusal {
    pub host: conduit_std_host::StdHost,
    pub equipment: DetachedCollectorEquipment,
    pub reason: Refusal,
}
impl std::fmt::Debug for DetachRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetachRefusal")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for ResumeRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResumeRefusal")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
impl CollectorRoot {
    /// Seal before publication and move the actual idle Host out without cloning
    /// it, its Play allocator, or its ledger. All native providers stay retained.
    #[allow(clippy::result_large_err)]
    pub fn detach(
        mut self,
    ) -> Result<(conduit_std_host::StdHost, DetachedCollectorEquipment), DetachRefusal> {
        let prepared = (|| {
            let inventory = self.seal_inventory()?;
            if self.slots.iter().any(|slot| slot.provider.is_none()) {
                return Err(Refusal::Possession);
            }
            if !self
                .installed
                .lock()
                .expect("collector Host mutex poisoned")
                .as_ref()
                .ok_or(Refusal::Possession)?
                .native_resources_idle()
            {
                return Err(Refusal::Capacity);
            }
            Ok(inventory)
        })();
        let inventory = match prepared {
            Ok(inventory) => inventory,
            Err(reason) => {
                return Err(DetachRefusal {
                    collector: self,
                    reason,
                })
            }
        };
        let host = self
            .installed
            .lock()
            .expect("collector Host mutex poisoned")
            .take()
            .expect("validated actual owner");
        Ok((
            host,
            DetachedCollectorEquipment {
                collector: self,
                inventory,
            },
        ))
    }
}
impl DetachedCollectorEquipment {
    /// Return the exact original Host after ordinary installed-Root execution.
    /// Refusals return both affine owners; no inventory or issuer is reconstructed.
    #[allow(clippy::result_large_err)]
    pub fn resume(self, host: conduit_std_host::StdHost) -> Result<CollectorRoot, ResumeRefusal> {
        let reason = if self
            .collector
            .installed
            .lock()
            .expect("collector Host mutex poisoned")
            .is_some()
        {
            Some(Refusal::Possession)
        } else if !host.native_resources_idle() {
            Some(Refusal::Capacity)
        } else if host.advertisement() != &self.inventory {
            Some(Refusal::Advertisement)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(ResumeRefusal {
                host,
                equipment: self,
                reason,
            });
        }
        *self
            .collector
            .installed
            .lock()
            .expect("collector Host mutex poisoned") = Some(host);
        Ok(self.collector)
    }
}

#[cfg(test)]
mod handoff_tests {
    use super::*;
    #[test]
    fn actual_ledger_lease_refuses_both_handoff_directions_without_losing_owners() {
        let source = super::super::tests::fixture();
        let fragment = source.plan().unwrap().fragments.remove(0);
        let installed = source.installed.clone();
        let collector = CollectorRoot {
            installed: installed.clone(),
            slots: Vec::new(),
            sealed: false,
        };
        let lease = installed
            .lock()
            .expect("collector Host mutex poisoned")
            .as_mut()
            .unwrap()
            .reserve_native_fragment(&fragment)
            .unwrap();
        let refusal = match collector.detach() {
            Err(refusal) => refusal,
            Ok(_) => panic!("active lease detached"),
        };
        assert_eq!(refusal.reason, Refusal::Capacity);
        installed
            .lock()
            .expect("collector Host mutex poisoned")
            .as_mut()
            .unwrap()
            .release_native_fragment(lease)
            .unwrap();
        let (mut host, equipment) = refusal.collector.detach().unwrap();
        assert!(installed
            .lock()
            .expect("collector Host mutex poisoned")
            .is_none());
        let lease = host.reserve_native_fragment(&fragment).unwrap();
        let mut refusal = match equipment.resume(host) {
            Err(refusal) => refusal,
            Ok(_) => panic!("active lease resumed"),
        };
        assert_eq!(refusal.reason, Refusal::Capacity);
        assert!(installed
            .lock()
            .expect("collector Host mutex poisoned")
            .is_none());
        refusal.host.release_native_fragment(lease).unwrap();
        let collector = refusal.equipment.resume(refusal.host).unwrap();
        assert!(installed
            .lock()
            .expect("collector Host mutex poisoned")
            .is_some());
        assert_eq!(
            collector.advertisement().unwrap(),
            source.advertisement().unwrap()
        );
    }
}

#[cfg(test)]
const _: fn() = || {
    fn send<T: Send>() {}
    send::<conduit_std_host::StdHost>();
    send::<DetachedCollectorEquipment>();
};
