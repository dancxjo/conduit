//! Raspberry Pi native Root: retained hardware, checked USB plots, shared mouse input.
//! All USB/class/report meaning lives in the portable protocol Source stages.
use crate::{
    arch,
    arch::dwc2::{EndpointRoute, TransactionTranslator, TransferType},
    identity::BootIdentities,
    make::{EMBEDDED_MAKE, IMPL_USB_TRANSPORT},
    monotonic_clock::{installation::ReadyClockBase, play::ClockPlay},
    usb_base::{
        dwc2_bus_route::BusControlReservation,
        dwc2_control::RootControlRoute,
        dwc2_control_play::Dwc2ControlPlay,
        dwc2_endpoint_read::RootEndpointRoute,
        dwc2_hub_play::Dwc2HubPlay,
        dwc2_pointer_play::Dwc2PointerPlay,
        dwc2_pointer_session::{Dwc2PointerSession, PointerSessionRefusal},
    },
};
use alloc::{boxed::Box, format};
use conduit_core::*;
use conduit_semantic_catalog::NormalizedPointerSample;
#[path = "armv6_usb_input/admission.rs"]
mod admission;
#[path = "armv6_usb_input/clock.rs"]
mod clock;
type Hub = Dwc2HubPlay<arch::BcmUsbRegisters, clock::Counter>;
type Pointer = Dwc2PointerSession<arch::BcmUsbRegisters>;
/// Neither this owner nor its fenced native transfers may be dropped on a
/// timeout/loss path. Product Root retains it until physical retirement.
pub struct RaspberryPiUsbInput {
    ids: BootIdentities,
    entropy: admission::Entropy,
    hub: Option<Box<Hub>>,
    pointer: Option<Box<Pointer>>,
    stopped: bool,
    pointer_started: bool,
    interval_ms: u64,
    next_read_ms: u64,
}
impl RaspberryPiUsbInput {
    pub fn prepare(ids: &BootIdentities) -> Result<Self, &'static str> {
        let expected_target = if cfg!(feature = "aarch64-rpi-3b-plus") {
            "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3"
        } else {
            "conduitos/armv6/raspberry-pi-model-b-plus-v1.2"
        };
        if !EMBEDDED_MAKE.includes(IMPL_USB_TRANSPORT) || EMBEDDED_MAKE.target != expected_target {
            return Err("usb-not-selected-by-make");
        }
        let board = if cfg!(feature = "aarch64-rpi-3b-plus") {
            arch::raspberry_pi::Board::ThreeBPlus
        } else {
            arch::raspberry_pi::Board::BPlus
        };
        if !arch::firmware_board_revision().is_some_and(|revision| board.accepts_revision(revision))
        {
            return Err("usb-board-does-not-match-make");
        }
        let host = arch::BcmUsbHost::prepare()?;
        let facts = host.facts();
        arch::present(
            format!(
                "CONDUIT_USB_CONTROLLER core={:#x} channels={} fifo_words={} root_port={:#x}\n",
                facts.core_id, facts.channels, facts.fifo_words, facts.root_port
            )
            .as_bytes(),
        );
        if (facts.root_port >> 17) & 3 != 0 {
            return Err("usb-root-link-not-high-speed");
        }
        let controller = host.into_controller();
        let mut entropy =
            admission::Entropy::admit(arch::HardwareEntropy).map_err(|e| e.as_str())?;
        let identity = admission::usb_identity(ids, "control");
        // SAFETY: this Root owns the original prepared controller, complete
        // reserved bus namespace, electrical permission and native PIO storage.
        let issuer = unsafe { admission::usb_issuer(&identity, false, &mut entropy) }?;
        let reservation = unsafe { BusControlReservation::admit_owned_hub_tree(1, 2) }
            .ok_or("usb-bus-reservation")?;
        let route = RootControlRoute {
            slot: 1,
            generation: 1,
            address: 0,
            maximum_packet: 64,
            translator: None,
        };
        let control = unsafe {
            Dwc2ControlPlay::prepare_bus(controller, route, reservation, &identity, issuer)
        }
        .map_err(|e| {
            observe("control-prepare", &e);
            "usb-control-prepare"
        })?;
        let clock_identity = admission::clock_identity(ids);
        // SAFETY: the same Root owns the calibrated counter and one-shot wake
        // path. Its provider performs one observation per admitted service step.
        let clock_issuer = unsafe { admission::clock_issuer(&clock_identity, &mut entropy) }?;
        let ready = unsafe { ReadyClockBase::new(clock_identity.clone(), clock::Counter::new()) }
            .map_err(|_| "usb-clock-base")?;
        let clock = ClockPlay::prepare(ready, &clock_identity, clock_issuer).map_err(|e| {
            observe("clock-prepare", &e);
            "usb-clock-prepare"
        })?;
        arch::present(b"CONDUIT_BOOT_STAGE usb-source-prepare\n");
        let mut hub = Dwc2HubPlay::prepare(
            control,
            clock,
            identity.host_id,
            identity.boot_id,
            route,
            1,
            2,
        )
        .map_err(|e| {
            observe("hub-prepare", &e);
            "usb-source-prepare"
        })?;
        hub.start().map_err(|e| {
            observe("hub-start", &e);
            "usb-source-start"
        })?;
        arch::present(b"CONDUIT_BOOT_STAGE usb-hub-play\n");
        Ok(Self {
            ids: *ids,
            entropy,
            hub: Some(Box::new(hub)),
            pointer: None,
            stopped: false,
            pointer_started: false,
            interval_ms: 1,
            next_read_ms: 0,
        })
    }
    pub fn ready(&self) -> bool {
        self.pointer.is_some() && !self.stopped
    }
    /// Root finishes graphical admission before starting this finite input Play.
    pub fn start_pointer(&mut self) -> Result<(), &'static str> {
        if self.stopped || self.pointer_started {
            return Err("usb-pointer-start-state");
        }
        self.pointer
            .as_mut()
            .ok_or("usb-pointer-not-prepared")?
            .start()
            .map_err(|e| {
                observe("pointer-start", &e);
                "usb-pointer-start"
            })?;
        self.pointer_started = true;
        Ok(())
    }
    pub fn needs_tick(&self) -> bool {
        if self.stopped {
            return true;
        }
        self.hub.as_ref().is_some_and(|hub| hub.waiting_for_clock())
            || self.pointer.as_ref().is_some_and(|pointer| {
                pointer.ready() && arch::read_counter() / 1000 < self.next_read_ms
            })
    }
    pub fn step(&mut self) -> Result<Option<NormalizedPointerSample>, &'static str> {
        if self.stopped {
            return Ok(None);
        }
        let result = self.step_owned();
        if result.is_err() {
            self.stopped = true;
        }
        result
    }
    fn step_owned(&mut self) -> Result<Option<NormalizedPointerSample>, &'static str> {
        let now = arch::read_counter() / 1000;
        if let Some(hub) = self.hub.as_mut() {
            if !hub.step(now).map_err(|e| {
                observe("hub-step", &e);
                "usb-enumeration-refused"
            })? {
                return Ok(None);
            }
            // Preparation starts only after the prior Source stages have closed
            // normally. The original controller remains retained on refusal.
            let result = hub.result().to_vec();
            let value = validate_canonical_structured_value(&result)
                .map_err(|_| "usb-enumeration-value")?;
            if let Some(refused) = value
                .variant_payload("refused")
                .map_err(|_| "usb-enumeration-value")?
            {
                use core::fmt::Write;
                let reason_value = field(refused, "reason")?;
                let reason = reason_value
                    .variant_tag()
                    .map_err(|_| "usb-enumeration-value")?;
                let mut text = crate::sign_format::FixedText::new();
                let _ = writeln!(
                    text,
                    "CONDUIT_USB_ENUMERATION_RESULT port={} reason={reason}",
                    octet(refused, "port")?
                );
                arch::present(text.as_bytes());
                return Err(if reason == "no-input" {
                    "usb-no-supported-pointer"
                } else {
                    "usb-hub-policy-refused"
                });
            }
            let selected = value
                .variant_payload("input")
                .map_err(|_| "usb-enumeration-value")?
                .ok_or("usb-no-supported-pointer")?;
            let translator_port = octet(selected, "translator-port")?;
            let translator = if translator_port == 0 {
                None
            } else {
                Some(TransactionTranslator {
                    hub_address: octet(selected, "hub-address")?,
                    port: translator_port,
                    low_speed: field(selected, "low-speed")?
                        .primitive_bytes("value/bool")
                        .map_err(|_| "usb-route-value")?
                        == [1],
                })
            };
            let maximum_packet =
                u8::try_from(word(selected, "maximum-packet")?).map_err(|_| "usb-packet-value")?;
            let route = RootEndpointRoute {
                slot: 1,
                generation: 1,
                endpoint_generation: 1,
                endpoint: EndpointRoute {
                    address: octet(selected, "address")?,
                    endpoint: octet(selected, "endpoint")?,
                    maximum_packet,
                    transfer_type: TransferType::Interrupt,
                    translator,
                },
            };
            self.interval_ms = word(selected, "interval-ms")?;
            if self.interval_ms == 0 {
                return Err("usb-zero-polling-interval");
            }
            let identity = admission::usb_identity(&self.ids, "pointer");
            // SAFETY: original settled controller and actually configured
            // endpoint; only this Root can issue the corresponding native scope.
            let issuer = unsafe { admission::usb_issuer(&identity, true, &mut self.entropy) }?;
            let hub = self.hub.take().expect("original hub owner");
            let controller = match (*hub).retire_controller() {
                Ok(controller) => controller,
                Err((hub, error)) => {
                    observe("hub-retire", &error);
                    self.hub = Some(Box::new(hub));
                    return Err("usb-controller-retirement-refused");
                }
            };
            let (play, schemas) =
                unsafe { Dwc2PointerPlay::prepare(controller, route, &identity, issuer) }.map_err(
                    |e| {
                        observe("pointer-prepare", &e);
                        "usb-pointer-prepare"
                    },
                )?;
            let pointer = Dwc2PointerSession::prepare(play, schemas, selected).map_err(|e| {
                observe("pointer-context", &e);
                "usb-pointer-context"
            })?;
            self.pointer = Some(Box::new(pointer));
            self.next_read_ms = now;
            arch::present(b"CONDUIT_USB_INPUT_READY source=usb-hid-absolute-device physical_input_proved=false\n");
            return Ok(None);
        }
        if !self.pointer_started {
            return Ok(None);
        }
        let pointer = self.pointer.as_mut().ok_or("usb-input-owner-absent")?;
        let sample = pointer.step().map_err(|e| {
            observe("pointer-step", &e);
            "usb-pointer-refused"
        })?;
        if pointer.ready() && now >= self.next_read_ms {
            match pointer.read() {
                Ok(()) => {
                    self.next_read_ms = now
                        .checked_add(self.interval_ms)
                        .ok_or("usb-counter-bound")?
                }
                Err(PointerSessionRefusal::Pressure) => {}
                Err(e) => {
                    observe("pointer-read", &e);
                    return Err("usb-pointer-read");
                }
            }
        }
        Ok(sample)
    }
}
fn observe(stage: &str, error: &impl core::fmt::Debug) {
    use core::fmt::Write;
    let mut text = crate::sign_format::FixedText::new();
    let _ = writeln!(
        &mut text,
        "CONDUIT_USB_REFUSAL stage={stage} detail={error:?}"
    );
    arch::present(text.as_bytes());
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, &'static str> {
    value
        .record_field(name)
        .map_err(|_| "usb-route-value")?
        .ok_or("usb-route-field")
}
fn octet(value: ValidatedCanonicalStructuredValue<'_>, name: &str) -> Result<u8, &'static str> {
    let value = field(value, name)?
        .primitive_bytes("value/u8")
        .map_err(|_| "usb-route-octet")?;
    value.first().copied().ok_or("usb-route-octet")
}
fn word(value: ValidatedCanonicalStructuredValue<'_>, name: &str) -> Result<u64, &'static str> {
    let value = field(value, name)?
        .primitive_bytes("value/u64")
        .map_err(|_| "usb-route-word")?;
    Ok(u64::from_le_bytes(
        value.try_into().map_err(|_| "usb-route-word")?,
    ))
}
