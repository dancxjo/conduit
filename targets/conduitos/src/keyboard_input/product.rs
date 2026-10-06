//! Ordinary physical input polling and bounded product service opportunities.
//! Ingress owns portable queue admission; the callback owns product interactions.
use super::{INGRESS_CAPACITY, KeyboardIngress, KeyboardIngressRefusal};
use crate::arch::{HidKeyboardSession, UsbDevice, XhciReady};
use conduit_human::KeyEvent;

/// Runs the ordinary physical input source for a long-lived product-owned
/// Play. Physical transitions cross the local-rescue boundary while their
/// provenance is still known, then are canonicalized into the exact finite
/// `input/keyboard` queue. The next USB transfer is armed before a bounded
/// semantic/presentation service slice drains portable `KeyEvent` values.
pub fn run_product(
    session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    device: &UsbDevice,
    mut interact: impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<(), &'static str> {
    let mut ingress = KeyboardIngress::new();
    for transition in session.transitions().iter().copied() {
        if interact(ProductInputEvent::LocalRescue(
            transition.into_local_rescue(),
        ))? == ProductInputControl::Yield
        {
            return Ok(());
        }
    }
    ingress
        .admit_report(session.transitions())
        .map_err(KeyboardIngressRefusal::as_str)?;

    loop {
        // Publish the next transfer before giving admitted semantic work or
        // presentation/export a turn.
        if !session.followup_pending() {
            session
                .begin_followup(controller, device)
                .map_err(|error| error.as_str())?;
        }
        if service_product_ingress(&mut ingress, &mut interact)? == ProductInputControl::Yield {
            return Ok(());
        }

        let (transitions, count) = loop {
            match session.poll_followup(controller, device) {
                Ok(Some(batch)) => break batch,
                Ok(None) => {
                    if service_product_ingress(&mut ingress, &mut interact)?
                        == ProductInputControl::Yield
                    {
                        return Ok(());
                    }
                    core::hint::spin_loop();
                }
                Err(error) => {
                    let reason = error.as_str();
                    interact(ProductInputEvent::Lost(reason))?;
                    return Err(reason);
                }
            }
        };

        for transition in transitions[..count].iter().copied() {
            if interact(ProductInputEvent::LocalRescue(
                transition.into_local_rescue(),
            ))? == ProductInputControl::Yield
            {
                return Ok(());
            }
        }
        // Whole-report admission is atomic. We intentionally do not run Plot
        // work here: the loop immediately publishes the next receive transfer,
        // then services these portable events within the admitted bound.
        ingress
            .admit_report(&transitions[..count])
            .map_err(KeyboardIngressRefusal::as_str)?;
    }
}

/// Runs the same long-lived portable keyboard source from a validated PS/2
/// mechanism. The adapter polls only a finite controller-byte window before a
/// bounded semantic service slice, retaining decoder prefixes between turns.
pub fn run_ps2_product(
    input: &mut crate::arch::Ps2Input,
    mut interact: impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<(), &'static str> {
    let mut ingress = KeyboardIngress::new();
    loop {
        for _ in 0..INGRESS_CAPACITY {
            match input.poll_keyboard() {
                Ok(Some(transition)) => {
                    if interact(ProductInputEvent::LocalRescue(
                        transition.into_local_rescue(),
                    ))? == ProductInputControl::Yield
                    {
                        return Ok(());
                    }
                    ingress
                        .admit(transition)
                        .map_err(KeyboardIngressRefusal::as_str)?;
                }
                Ok(None) => break,
                Err(error) => {
                    let reason = error.as_str();
                    interact(ProductInputEvent::Lost(reason))?;
                    return Err(reason);
                }
            }
        }
        if service_product_ingress(&mut ingress, &mut interact)? == ProductInputControl::Yield {
            return Ok(());
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductInputControl {
    Continue,
    Yield,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductInputEvent {
    /// Host service opportunity, distinct from a physical input transition.
    Service,
    /// Proven-local physical transition for the narrow rescue policy only.
    LocalRescue(crate::local_rescue::ValidatedLocalTransition),
    /// Portable semantic keyboard occurrence drained from admitted ingress.
    Key(KeyEvent),
    Lost(&'static str),
}

/// Deliver at most the exact admitted keyboard queue capacity. If any keyboard
/// work ran, return immediately so the caller can re-check the physical source;
/// presentation/export service runs only from an already-idle ingress turn.
fn service_product_ingress(
    ingress: &mut KeyboardIngress,
    interact: &mut impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<ProductInputControl, &'static str> {
    let mut delivered = false;
    for _ in 0..INGRESS_CAPACITY {
        let Some(event) = ingress.take() else {
            break;
        };
        delivered = true;
        if interact(ProductInputEvent::Key(event))? == ProductInputControl::Yield {
            return Ok(ProductInputControl::Yield);
        }
    }
    if delivered {
        Ok(ProductInputControl::Continue)
    } else {
        interact(ProductInputEvent::Service)
    }
}

/// Service a retained Source batch whose physical provenance Root has already
/// established from its admitted USB capture. The caller retains ingress and
/// batch across Yield/pressure and acknowledges Source only after it is empty.
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)] // Root Source capture composition is being connected.
pub(crate) fn service_source_batch(
    batch: &mut crate::source_keyboard_batch::PendingSourceKeyboardBatch,
    ingress: &mut KeyboardIngress,
    interact: &mut impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<ProductInputControl, &'static str> {
    for _ in 0..INGRESS_CAPACITY {
        let Some(transition) = batch.next_transition() else {
            break;
        };
        match batch.admit_next(ingress) {
            Ok(true) => {
                if interact(ProductInputEvent::LocalRescue(
                    transition.into_local_rescue(),
                ))? == ProductInputControl::Yield
                {
                    return Ok(ProductInputControl::Yield);
                }
            }
            Ok(false) => break,
            Err(KeyboardIngressRefusal::Pressure) => break,
            Err(error) => return Err(error.as_str()),
        }
    }
    service_product_ingress(ingress, interact)
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
