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
    standalone_publication: bool,
}
impl CollectorRoot {
    pub fn new(installed: conduit_std_host::StdHost) -> Self {
        Self {
            installed: Arc::new(Mutex::new(Some(installed))),
            slots: Vec::with_capacity(4),
            sealed: false,
            standalone_publication: true,
        }
    }
    /// Installed execution exposes only Backs dispatched by the ordinary kernel.
    pub fn new_installed(installed: conduit_std_host::StdHost) -> Self {
        let mut root = Self::new(installed);
        root.standalone_publication = false;
        root
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
            |provider, identity, base| {
                if !self.standalone_publication {
                    base.capabilities
                        .retain(|offer| offer.capability_id != identity.publication);
                }
                match provider {
                    Provider::Usb(_) => Ok(()),
                    Provider::Network(network) => network.register_native(identity, base),
                }
            },
        )?;
        let id = CollectorSourceId(self.slots.len());
        self.slots.push(root);
        Ok(id)
    }
    /// Locally owned planning grants; copying them establishes no remote membership.
    pub fn source_authority_grants(&self) -> Vec<AuthorityGrant> {
        self.slots.iter().map(UsbSourceRoot::grant).collect()
    }
    pub fn native_authority_grants(&self) -> Result<Vec<AuthorityGrant>, Refusal> {
        let host = self.advertisement()?;
        let mut grants = self.source_authority_grants();
        for slot in &self.slots {
            if let Some(Provider::Network(network)) = slot.provider.as_ref() {
                grants.extend(network.authority_grants(&host));
            }
        }
        Ok(grants)
    }
    pub fn source_descriptor(
        &self,
        id: CollectorSourceId,
    ) -> Result<SourceSessionDescriptor, Refusal> {
        self.slots
            .get(id.0)
            .map(UsbSourceRoot::source_descriptor)
            .ok_or(Refusal::Possession)
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
        if !self.standalone_publication {
            return Err(Refusal::Authority);
        }
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
            standalone_publication: true,
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

impl DetachedCollectorEquipment {
    /// Compatibility caller: prepare one original-lease work owner, retire native
    /// IO, then reunite and complete the same ordinary kernel request.
    pub async fn complete_admitted_request(
        self,
        host: conduit_std_host::StdHost,
        mut admitted: conduit_std_host::AdmittedRemoteFragment,
        request: conduit_kernel::scheduler::HostCallRequest,
    ) -> (
        conduit_std_host::StdHost,
        Self,
        conduit_std_host::AdmittedRemoteFragment,
        Result<bool, Refusal>,
    ) {
        let work = match host.prepare_native_work(&mut admitted, request) {
            Ok(work) => work,
            Err(_) => return (host, self, admitted, Err(Refusal::Authority)),
        };
        let (host, equipment, work, captured) = self.complete_native_work(host, work).await;
        let returned = match host.return_native_work(&mut admitted, work) {
            Ok(returned) => returned,
            Err(_) => return (host, equipment, admitted, Err(Refusal::Authority)),
        };
        let completed = returned
            .complete(&mut admitted, captured.as_ref().ok().map(Vec::as_slice))
            .map_err(|_| Refusal::Authority);
        let result = match captured {
            Ok(_) => completed,
            Err(reason) => Err(reason),
        };
        (host, equipment, admitted, result)
    }
    /// Worker owns the actual Host, native equipment and ORIGINAL affine lease.
    /// The ordinary kernel/session stays on the control thread. No lease is
    /// copied, released, reconstructed or reserved a second time by this turn.
    pub async fn complete_native_work(
        self,
        host: conduit_std_host::StdHost,
        mut work: conduit_std_host::PreparedNativeWork,
    ) -> (
        conduit_std_host::StdHost,
        Self,
        conduit_std_host::PreparedNativeWork,
        Result<Vec<u8>, Refusal>,
    ) {
        if host.advertisement() != &self.inventory
            || self
                .collector
                .installed
                .lock()
                .expect("collector Host mutex poisoned")
                .is_some()
            || host.validate_native_work(&work).is_err()
        {
            return (host, self, work, Err(Refusal::Advertisement));
        }
        let selected = work.selected();
        let Some(operation) = selected.lowered.host_calls.iter().find(|operation| {
            operation.node == work.request().node && operation.call == work.request().call
        }) else {
            return (host, self, work, Err(Refusal::Plan));
        };
        let Some(gear) = selected
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
        else {
            return (host, self, work, Err(Refusal::Plan));
        };
        if operation.contract_id.as_str()
            == conduit_std_host::isolated_http_base::HTTP_CLIENT_OPERATION
        {
            let slot=self.collector.slots.iter().find(|slot|matches!(slot.provider.as_ref(),Some(Provider::Network(network)) if network.matches_capability(&gear.capability_id)));
            let result = match slot {
                Some(slot) if !slot.failed => match slot.provider.as_ref() {
                    Some(Provider::Network(network)) => network.exchange_admitted(&host, &mut work),
                    _ => Err(Refusal::Possession),
                },
                _ => Err(Refusal::Possession),
            };
            return (host, self, work, result);
        }
        if operation.contract_id.as_str() != HOST_CALL {
            return (host, self, work, Err(Refusal::Authority));
        }
        let Some(index) = self
            .collector
            .slots
            .iter()
            .position(|slot| slot.identity.capability == gear.capability_id)
        else {
            return (host, self, work, Err(Refusal::Possession));
        };
        if host.begin_native_work(&mut work).is_err() {
            return (host, self, work, Err(Refusal::Authority));
        }
        *self
            .collector
            .installed
            .lock()
            .expect("collector Host mutex poisoned") = Some(host);
        let mut equipment = self;
        let result = async {
            let selected = work.selected();
            let slot = &mut equipment.collector.slots[index];
            if slot.failed {
                return Err(Refusal::Possession);
            }
            let (table, handle, claim) =
                slot.issue_for_active(selected.fragment, selected.active)?;
            let contract = ObservationContract::prepare().map_err(|_| Refusal::Plan)?;
            let provider = slot.provider.take().ok_or(Refusal::Possession)?;
            // Actual initialized custody remains retained here; the opaque work
            // owner carries the original admitted lease and selected request.
            let binding = unsafe {
                EnvironmentalSourceHostCall::bind_selected_request(
                    table,
                    handle,
                    claim,
                    provider,
                    &contract,
                    selected,
                    work.request(),
                )
            };
            let mut owner = match binding {
                Ok(owner) => owner,
                Err((_, _, _, provider, _)) => {
                    slot.provider = Some(provider);
                    return Err(Refusal::Authority);
                }
            };
            let request = work.request();
            let mut bytes = [0u8; MAXIMUM_BYTES];
            let captured = owner
                .invoke(
                    request.node,
                    request.call,
                    request.request,
                    work.input(),
                    &mut bytes,
                )
                .await
                .map_err(|_| Refusal::Acquisition);
            let revoked = owner.revoke().map_err(|_| Refusal::Authority);
            let mut retained = Some(owner);
            slot.provider = Some(
                EnvironmentalSourceHostCall::retire_retained(&mut retained)
                    .map_err(|_| Refusal::Possession)?,
            );
            let length = match captured {
                Ok(length) => length,
                Err(reason) => {
                    slot.failed = true;
                    return Err(reason);
                }
            };
            revoked?;
            Ok(bytes[..length].to_vec())
        }
        .await;
        let host = equipment
            .collector
            .installed
            .lock()
            .expect("collector Host mutex poisoned")
            .take()
            .expect("actual owner retained through native turn");
        (host, equipment, work, result)
    }
}
