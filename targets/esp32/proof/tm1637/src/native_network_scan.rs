//! Exclusive scan preparation from the actual returned AP owner. No parallel
//! radio slot, SDK reservation, service allowance or generation is introduced.
use super::*;
use alloc::boxed::Box;
use conduit_esp32_network_switch::Esp32NetworkScanProvider;
use conduit_network_join_call::scan::ReadyNetworkScanBase;

pub(crate) type ScanProvider = Esp32NetworkScanProvider<'static, 'static, crate::services::Owner>;

impl NativeNetwork {
    /// Reserve fallible descriptive/backing memory before transferring the AP.
    /// The caller must independently admit this transient workspace and then
    /// select the returned owner through Root's exact scan Source and issuance.
    /// Merely holding this value cannot invoke the provider.
    pub(crate) fn prepare_scan(
        &mut self,
        host: &c::HostId,
        boot: &c::BootId,
        now_ms: u64,
    ) -> Result<Box<ReadyNetworkScanBase<ScanProvider>>, &'static str> {
        if self.selected
            || self.ready.is_some()
            || self.adapter.is_none()
            || self.radio.is_some()
            || self.services.is_some()
            || self.configuration.is_some()
            || self.retirement.is_some()
            || self.generation == 0
            || self.generation == u64::MAX
            || self.sdk.bytes() != crate::sdk_memory::SDK_BYTES
            || host.as_str().is_empty()
            || boot.as_str().is_empty()
        {
            return Err("scan-restored-owner-unavailable");
        }
        let request = conduit_net::NetworkScanRequest {
            generation: self.generation,
            scan_deadline_ms: now_ms.checked_add(15_000).ok_or("scan-clock-exhausted")?,
            cleanup_deadline_ms: now_ms.checked_add(20_000).ok_or("scan-clock-exhausted")?,
        };
        request.validate().map_err(|_| "scan-request-invalid")?;
        // Same physical Base, instance, resource and generation as join. The
        // exact scan operation/subject/profile are checked separately by Core.
        let identity = crate::native_network_identity::prepare(
            crate::native_network_identity::Role::Station,
            host,
            boot,
            self.generation,
        )
        .map_err(|_| "scan-native-identity-capacity")?;
        let backing =
            crate::native_box_backing::reserve_box().map_err(|_| "scan-ready-owner-capacity")?;
        // Every physical refusal leaves the original adapter in its slot. This
        // does not stop AP services, issue a command or start an SDK scan.
        let provider = Esp32NetworkScanProvider::prepare_from_restored(&mut self.adapter)
            .map_err(|_| "scan-provider-preparation-refused")?;
        // SAFETY: the single actual controller/services were just transferred
        // from our restored AP slot. Root retains its independent SDK backing;
        // finite absolute deadlines above are never extended on cancellation.
        let ready = match unsafe { ReadyNetworkScanBase::new(identity, request, provider) } {
            Ok(ready) => ready,
            Err((_, _, provider)) => {
                // Identity/request were validated before transfer. Still return
                // actual physical ownership rather than lose it on this branch.
                match provider.try_into_restored() {
                    Ok(adapter) => self.adapter = Some(adapter),
                    Err(_) => unreachable!("untouched prepared scan AP owner"),
                }
                return Err("scan-ready-identity-invalid");
            }
        };
        let ready = Box::write(backing, ready);
        // This internal flag excludes another radio handoff; it does not report
        // a Body as Playing and does not constitute issued scan possession.
        self.selected = true;
        Ok(ready)
    }

    /// After real selected-operation retirement, recover only the actual AP
    /// controller and current service acknowledgment. Evidence commitment and
    /// policy renewal remain separate obligations of the selected scan caller.
    pub(crate) fn reclaim_restored_scan(
        &mut self,
        provider: ScanProvider,
    ) -> Result<(), ScanProvider> {
        if !self.can_reclaim_restored() || self.retirement.is_some() {
            return Err(provider);
        }
        let next = self.generation.checked_add(1).expect("checked generation");
        match provider.try_into_restored() {
            Ok(adapter) => {
                self.adapter = Some(adapter);
                self.generation = next;
                self.selected = false;
                Ok(())
            }
            Err(provider) => Err(provider),
        }
    }
}

impl NativeNetwork {
    /// Return an unissued scan preparation without spending its generation.
    /// The Ready type cannot invoke the provider; actual AP acknowledgment is
    /// still checked by the adapter before physical custody is restored.
    pub(crate) fn return_unselected_scan(
        &mut self,
        ready: ReadyNetworkScanBase<ScanProvider>,
    ) -> Result<(), ReadyNetworkScanBase<ScanProvider>> {
        if !self.can_reclaim_restored()
            || self.retirement.is_some()
            || ready.native_identity().provider_generation != self.generation
        {
            return Err(ready);
        }
        let (identity, request, provider) = ready.into_unbound_parts();
        match provider.try_into_restored() {
            Ok(adapter) => {
                self.adapter = Some(adapter);
                self.selected = false;
                Ok(())
            }
            Err(provider) => {
                // SAFETY: unchanged already-validated identity/request and same
                // exclusive provider; no issued operation was accepted here.
                Err(
                    unsafe { ReadyNetworkScanBase::new(identity, request, provider) }
                        .unwrap_or_else(|_| unreachable!("unchanged scan Ready")),
                )
            }
        }
    }
}
