//! Bounded delivery of existing keyboard Source batches over Root's one bus.
//! Report validation, modifiers and transition ordering belong to the plots.
use super::{
    dwc2_endpoint_read::RootEndpointRoute, dwc2_hid_play::*, installation::UsbNativeIdentity,
};
use crate::{
    arch::dwc2::{Controller, RegisterIo},
    keyboard_ingress::KeyboardIngress,
    protocol_source::NativeProtocolIssuer,
    source_keyboard_batch::{PendingSourceKeyboardBatch, SourceKeyboardBatchDecoder},
};
use alloc::vec::Vec;
use conduit_core::*;
use conduit_human::KeyEvent;

pub(crate) struct Dwc2KeyboardSession {
    play: Dwc2HidPlay,
    read: PortId,
    pulse: ValuePayload,
    outputs: Vec<(PortId, ValuePayload)>,
    decoder: SourceKeyboardBatchDecoder,
    pending: Option<PendingSourceKeyboardBatch>,
    ingress: KeyboardIngress,
    sequence: u64,
    working: bool,
    received: bool,
    observed: bool,
    changed: bool,
}
impl Dwc2KeyboardSession {
    /// # Safety
    /// Root retains the original controller and an independently admitted,
    /// configured boot-keyboard endpoint throughout this session's lifetime.
    pub(crate) unsafe fn prepare<I: RegisterIo>(
        controller: &Controller<I>,
        route: RootEndpointRoute,
        identity: &UsbNativeIdentity,
        issuer: NativeProtocolIssuer,
    ) -> Result<Self, HidPlayRefusal> {
        use HidPlayRefusal as E;
        let (mut play, schemas) =
            unsafe { Dwc2HidPlay::prepare(controller, true, route, identity, issuer) }?;
        let decoder =
            SourceKeyboardBatchDecoder::prepare(&schemas.update).map_err(|_| E::Binding)?;
        let boundary = &play.kernel_mut().definition().boundary;
        let port = |name: &str| {
            boundary
                .input_fronts
                .iter()
                .find(|p| p.external_port.port_id.as_str() == name)
                .map(|p| p.external_port.clone())
                .ok_or(E::Binding)
        };
        let begin = port("begin")?;
        let read = port("read")?;
        let outputs = boundary
            .output_fronts
            .iter()
            .map(|p| {
                (
                    p.external_port.port_id.clone(),
                    ValuePayload {
                        value_kind: p.external_port.value_kind.clone(),
                        encoded: Vec::with_capacity(4096),
                    },
                )
            })
            .collect();
        let pulse = ValuePayload {
            value_kind: read.value_kind,
            encoded: Vec::new(),
        };
        play.kernel_mut().start().map_err(|_| E::Binding)?;
        if !matches!(
            play.kernel_mut()
                .admit_input(
                    &begin.port_id,
                    0,
                    &ValuePayload {
                        value_kind: begin.value_kind,
                        encoded: Vec::new(),
                    }
                )
                .map_err(|_| E::Binding)?,
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err(E::Binding);
        }
        // Keep begin open: closing it would finish the finite class lifecycle.
        Ok(Self {
            play,
            read: read.port_id,
            pulse,
            outputs,
            decoder,
            pending: None,
            ingress: KeyboardIngress::new(),
            sequence: 0,
            working: false,
            received: false,
            observed: false,
            changed: false,
        })
    }
    pub(crate) fn read(&mut self) -> Result<(), HidPlayRefusal> {
        use HidPlayRefusal as E;
        if !self.ready() {
            return Err(E::Binding);
        }
        let next = self.sequence.checked_add(1).ok_or(E::Binding)?;
        if !matches!(
            self.play
                .kernel_mut()
                .admit_input(&self.read, self.sequence, &self.pulse)
                .map_err(|_| E::Binding)?,
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err(E::Binding);
        }
        self.sequence = next;
        self.working = true;
        self.received = false;
        self.observed = false;
        self.changed = false;
        Ok(())
    }
    pub(crate) fn ready(&self) -> bool {
        !self.working && self.pending.is_none() && self.ingress.pending() == 0
    }
    pub(crate) fn take(&mut self) -> Option<KeyEvent> {
        self.ingress.take()
    }
    pub(crate) fn step<I: RegisterIo>(
        &mut self,
        controller: &mut Controller<I>,
    ) -> Result<(), HidPlayRefusal> {
        use HidPlayRefusal as E;
        if let Some(pending) = self.pending.as_mut() {
            while self.ingress.pending() < crate::keyboard_ingress::INGRESS_CAPACITY
                && pending.pending() != 0
            {
                pending
                    .admit_next(&mut self.ingress)
                    .map_err(|_| E::Binding)?;
            }
            if pending.pending() == 0 {
                self.pending = None;
            }
        }
        if self.play.step(controller)? != conduit_composite::KernelCompositeStatus::Active {
            return Err(E::Binding);
        }
        for (port, payload) in &mut self.outputs {
            let Some(sequence) = self
                .play
                .kernel_mut()
                .output_into(port, payload)
                .map_err(|_| E::Binding)?
            else {
                continue;
            };
            let value =
                validate_canonical_structured_value(&payload.encoded).map_err(|_| E::Binding)?;
            match port.as_str() {
                "received" => match value.variant_tag().map_err(|_| E::Binding)? {
                    "frame" => self.received = true,
                    "timeout" => self.working = false,
                    status => return Err(E::Report(source_status(status))),
                },
                "observed" => {
                    let status = value.variant_tag().map_err(|_| E::Binding)?;
                    if status != "keyboard" {
                        return Err(E::Report(source_status(status)));
                    }
                    self.observed = true;
                }
                "changes" => {
                    if self.pending.is_some() {
                        return Err(E::Binding);
                    }
                    self.pending = Some(
                        self.decoder
                            .decode(&payload.encoded)
                            .map_err(|_| E::Binding)?
                            .into_pending()
                            .map_err(|_| E::Binding)?,
                    );
                    self.changed = true;
                }
                _ => return Err(E::Binding),
            }
            self.play
                .kernel_mut()
                .complete_output(port, sequence)
                .map_err(|_| E::Binding)?;
        }
        if self.received && self.observed && self.changed {
            self.working = false;
        }
        Ok(())
    }
}

// Preserve checked Source result tags without interpreting any report bytes.
fn source_status(status: &str) -> &'static str {
    match status {
        "short" => "short",
        "malformed" => "malformed",
        "reserved" => "reserved",
        "report-error" => "report-error",
        "duplicate" => "duplicate",
        "stalled" => "stalled",
        "provider-lost" => "provider-lost",
        "unsupported" => "unsupported",
        _ => "unexpected-tag",
    }
}
