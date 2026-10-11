//! ISO shares the original controller/event owner and its acknowledged stop path.
//!
//! Root integration contract: prepare a checked operation's capability table,
//! exact handle/claim, provider/resource generations and byte/frame validators;
//! retain that original possession in the installed Host adapter throughout Play.
//! Configure Endpoint must return this move-only ISO receipt. Reserve one 64-TRB
//! ring, eight disjoint 256-byte packet slices and their eight-lease cursor before
//! Play. Root proves these 3072 DMA bytes disjoint from EP0 and every other owner,
//! plus input-context storage retained through Configure Endpoint acknowledgement.
//! Never bind a bulk receipt or infer authority from endpoint/address/epoch facts.
//!
//! Each Host Call first authorizes the exact live possession and validates its
//! typed octets/deadline. Only then may it invoke unsafe begin_isochronous. The
//! native owner checks the original attachment/endpoint epoch, actual controller
//! MFINDEX/IST, contiguous 1ms frames and finite eight-packet pressure. No sample
//! clock conversion or PCM interpretation occurs here. Root encodes the returned
//! native observations through the checked result Type: a submitted packet is
//! distinct from its USB completion, and neither proves audible output.
//!
//! Cancellation revokes the adapter's capability before invoking native stop.
//! Fault completion retains the exact pending claim until the typed terminal
//! settles; the native owner closes effects before it acknowledges stop, and
//! the adapter then retires possession. A failed acknowledgement retains every
//! outstanding DMA lease and admits
//! no further effect. Successful completion retires only its exact oldest TD;
//! missed service/short/underrun/loss requires acknowledged stop of all remaining
//! TDs before a terminal may be published. A drain is explicit: all submitted TDs
//! must complete, then the original controller must acknowledge Disable Slot.
//! Drop alone never permits DMA reclamation.
use super::*;
use crate::usb_base::isochronous_schedule::{IsochronousFrame, IsochronousOutObservation};
impl<'a> UsbNativeHost<'a> {
    /// Native-only observation, suitable for an admitted clock-domain projection.
    /// It never chooses a start frame, sample rate, jitter tolerance or retry.
    pub fn isochronous_snapshot(
        &self,
        endpoint_epoch: u64,
    ) -> Result<crate::usb_base::isochronous_schedule::IsochronousOutSnapshot, UsbNativeHostRefusal>
    {
        if self.stopped
            || self.cancellation_requested
            || self.stop_acknowledged
            || self.uncertain_control
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        let transfer = self
            .isochronous
            .as_ref()
            .ok_or(UsbNativeHostRefusal::Binding)?;
        transfer
            .validate(&self.device, endpoint_epoch)
            .map_err(UsbNativeHostRefusal::Isochronous)?;
        if self.controller.port_status(self.device.root_port) & 1 == 0 {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        Ok(
            crate::usb_base::isochronous_schedule::IsochronousOutSnapshot {
                current_microframe: self.controller.microframe_index(),
                scheduling_threshold: self.controller.isochronous_threshold(),
                service_interval_microframes: 8,
                maximum_packet_bytes: transfer.geometry().packet_bytes(),
                pending_packets: transfer.pending_count() as u8,
                maximum_pending_packets: 8,
                dma_resource_bytes: 3072,
            },
        )
    }

    /// # Safety
    /// Root independently admits this exact ISO endpoint, resource envelope and
    /// transfer schedule. The receipt and all coherent DMA are original and unique.
    /// No effects are authorized by descriptor fields or constructing this binding.
    pub unsafe fn add_isochronous(
        &mut self,
        configured: super::super::endpoint_setup::ConfiguredIsochronousOutboundEndpoint,
        dma: EndpointTransmitDma<'a>,
    ) -> Result<(), UsbNativeHostRefusal> {
        if self.playing
            || self.stopped
            || self.stop_acknowledged
            || self.cancellation_requested
            || self.isochronous.is_some()
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        // ISO is a separate reserved resource, never another view of an existing
        // bulk endpoint or its buffers. Root additionally proves all DMA disjoint.
        let transfer = unsafe {
            super::super::isochronous_out::IsochronousOutTransfer::bind(
                &self.device,
                configured,
                dma,
                self.mapping,
            )
        }
        .map_err(UsbNativeHostRefusal::Isochronous)?;
        self.isochronous = Some(transfer);
        Ok(())
    }
    /// # Safety
    /// The installed Root adapter validates the exact admitted call's possession,
    /// PCM-independent byte envelope and explicit clock-domain frame deadline before
    /// every submission. This native entry point does not reconstruct that authority.
    pub unsafe fn begin_isochronous(
        &mut self,
        endpoint_epoch: u64,
        frame: IsochronousFrame,
        input: &[u8],
    ) -> Result<(), UsbNativeHostRefusal> {
        if self.stopped
            || self.cancellation_requested
            || self.stop_acknowledged
            || self.pending_endpoint.is_some()
            || self.pending_write.is_some()
            || self
                .bulk_terminals
                .as_ref()
                .is_some_and(|slots| slots.iter().any(|slot| slot.index.is_some()))
            || self.uncertain_control
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        let transfer = self
            .isochronous
            .as_mut()
            .ok_or(UsbNativeHostRefusal::Binding)?;
        transfer
            .validate(&self.device, endpoint_epoch)
            .map_err(UsbNativeHostRefusal::Isochronous)?;
        transfer
            .begin(
                frame,
                input,
                self.controller.microframe_index(),
                self.controller.isochronous_threshold(),
            )
            .map_err(UsbNativeHostRefusal::Isochronous)?;
        self.isochronous_started = true;
        self.playing = true;
        self.controller
            .ring_endpoint(self.device.slot, transfer.dci());
        Ok(())
    }
    /// One actual event observation. No retry, fill, cadence repair or audio policy.
    pub fn poll_isochronous(
        &mut self,
        endpoint_epoch: u64,
    ) -> Result<Option<IsochronousOutObservation>, UsbNativeHostRefusal> {
        if self.stopped || self.stop_acknowledged || !self.isochronous_started {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        self.isochronous
            .as_ref()
            .ok_or(UsbNativeHostRefusal::Binding)?
            .validate(&self.device, endpoint_epoch)
            .map_err(UsbNativeHostRefusal::Isochronous)?;
        if self.controller.port_status(self.device.root_port) & 1 == 0 {
            self.cancel()?;
            return Ok(Some(IsochronousOutObservation::ProviderLost));
        }
        let Some(event) = self.controller.poll_event() else {
            return Ok(None);
        };
        let observation = match self
            .isochronous
            .as_mut()
            .ok_or(UsbNativeHostRefusal::Binding)?
            .event(event)
        {
            Ok(observation) => observation,
            Err(error) => {
                // An uncorrelated/failed event cannot authorize more DMA effects.
                // Keep all leases rooted; only acknowledged cancellation may retire.
                self.uncertain_control = true;
                return Err(UsbNativeHostRefusal::Isochronous(error));
            }
        };
        if matches!(
            observation,
            IsochronousOutObservation::Underrun
                | IsochronousOutObservation::MissedService { .. }
                | IsochronousOutObservation::Short { .. }
        ) {
            // Failed acknowledgement publishes no terminal and preserves DMA leases.
            self.cancel()?;
        }
        Ok(Some(observation))
    }
    /// Drain is requested by the admitted class plot. It waits for all actual TD
    /// completions and acknowledges slot stop before publishing the drain truth.
    pub fn drain_isochronous(
        &mut self,
        endpoint_epoch: u64,
    ) -> Result<IsochronousOutObservation, UsbNativeHostRefusal> {
        if self.stopped
            || self.cancellation_requested
            || self.stop_acknowledged
            || self.uncertain_control
            || !self.isochronous_started
        {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        let transfer = self
            .isochronous
            .as_ref()
            .ok_or(UsbNativeHostRefusal::Binding)?;
        transfer
            .validate(&self.device, endpoint_epoch)
            .map_err(UsbNativeHostRefusal::Isochronous)?;
        if transfer.pending_count() != 0 {
            return Err(UsbNativeHostRefusal::Pending);
        }
        self.cancel()?;
        Ok(IsochronousOutObservation::Drained)
    }
    pub fn cancel_isochronous(
        &mut self,
    ) -> Result<IsochronousOutObservation, UsbNativeHostRefusal> {
        if self.stopped || self.stop_acknowledged || self.isochronous.is_none() {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        self.cancel()?;
        Ok(IsochronousOutObservation::Cancelled)
    }
}
