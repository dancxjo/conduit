//! Ordinary physical input polling and bounded product service opportunities.
//! Ingress owns portable queue admission; the callback owns product interactions.
use super::{INGRESS_CAPACITY, KeyboardIngress, KeyboardIngressRefusal};
use crate::arch::{HidKeyboardSession, HidPointerSession, UsbDevice, XhciReady};
use conduit_human::KeyEvent;

/// Runs the ordinary physical input source for a long-lived product-owned
/// Play. Physical transitions cross the local-rescue boundary while their
/// provenance is still known, then are canonicalized into the exact finite
/// `input/keyboard` queue. The next USB transfer is armed before a bounded
/// semantic/face service slice drains portable `KeyEvent` values.
pub fn run_product(
    session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    device: &UsbDevice,
    interact: impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<(), &'static str> {
    run_product_with_pointer(session, controller, device, &mut None, interact)
}

/// Poll both admitted physical endpoints; neither adapter runs semantic work.
/// Each completed report is retained locally until both receives are rearmed.
pub fn run_product_with_pointer(
    session: &mut HidKeyboardSession,
    controller: &mut XhciReady,
    device: &UsbDevice,
    pointer: &mut Option<(&mut HidPointerSession, &UsbDevice)>,
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
    let mut pending_pointer = None;
    loop {
        if !session.followup_pending()
            && let Err(error) = session.begin_followup(controller, device)
        {
            interact(ProductInputEvent::Lost(error.as_str()))?;
            return Err(error.as_str());
        }
        let pointer_loss = if let Some((session, device)) = pointer.as_mut() {
            if !session.followup_pending() {
                session
                    .begin_followup(controller, device)
                    .err()
                    .map(|error| error.as_str())
            } else {
                None
            }
        } else {
            None
        };
        if pointer_loss.is_some() {
            *pointer = None;
        }
        if service_product_inputs(
            &mut ingress,
            &mut pending_pointer,
            pointer_loss,
            &mut interact,
        )? == ProductInputControl::Yield
        {
            return Ok(());
        }
        match session.poll_followup(controller, device) {
            Ok(Some((transitions, count))) => {
                // Rearm before provenance observations or semantic callbacks.
                session
                    .begin_followup(controller, device)
                    .map_err(|error| error.as_str())?;
                for transition in transitions[..count].iter().copied() {
                    if interact(ProductInputEvent::LocalRescue(
                        transition.into_local_rescue(),
                    ))? == ProductInputControl::Yield
                    {
                        return Ok(());
                    }
                }
                ingress
                    .admit_report(&transitions[..count])
                    .map_err(KeyboardIngressRefusal::as_str)?;
            }
            Ok(None) => {}
            Err(error) => {
                interact(ProductInputEvent::Lost(error.as_str()))?;
                return Err(error.as_str());
            }
        }
        if let Some((session, device)) = pointer.as_mut() {
            match session.poll_followup(controller, device) {
                Ok(sample) => pending_pointer = sample,
                Err(error) => {
                    let reason = error.as_str();
                    *pointer = None;
                    interact(ProductInputEvent::PointerLost(reason))?;
                }
            }
        }
        #[cfg(target_arch = "x86_64")]
        if ingress.pending() == 0 && pending_pointer.is_none() {
            crate::arch::idle_after_poll(controller);
        }
        core::hint::spin_loop();
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
    let mut pointer_available = true;
    loop {
        for _ in 0..INGRESS_CAPACITY {
            let keyboard = input.poll_keyboard();
            // Take each completed packet before another controller byte can
            // coalesce a down/up gesture into the final button level.
            if let Some(sample) = input.take_pending_pointer()
                && pointer_available
                && interact(ProductInputEvent::Pointer(sample))? == ProductInputControl::Yield
            {
                return Ok(());
            }
            match keyboard {
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
                Err(
                    error @ (crate::arch::Ps2Error::MalformedPointerPacket
                    | crate::arch::Ps2Error::PressureEvidenceOverflow),
                ) => {
                    if pointer_available {
                        pointer_available = false;
                        interact(ProductInputEvent::PointerLost(error.as_str()))?;
                    }
                }
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
    /// Validated physical sample for Mask-local cursor/gesture routing.
    Pointer(conduit_semantic_catalog::NormalizedPointerSample),
    /// Pointer retirement does not declare the keyboard or a Body input lost.
    PointerLost(&'static str),
}

/// Report loss before a previously completed sample can authorize an action.
/// Deliver that sample before keyboard service may yield to a finite Line.
fn service_product_inputs(
    ingress: &mut KeyboardIngress,
    pointer: &mut Option<conduit_semantic_catalog::NormalizedPointerSample>,
    pointer_loss: Option<&'static str>,
    interact: &mut impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
) -> Result<ProductInputControl, &'static str> {
    let mut yield_requested = false;
    if let Some(reason) = pointer_loss {
        // Finish this already-admitted physical batch before handing the
        // controller to a Line, while loss fences its possible press first.
        yield_requested =
            interact(ProductInputEvent::PointerLost(reason))? == ProductInputControl::Yield;
    }
    if let Some(sample) = pointer.take() {
        yield_requested |=
            interact(ProductInputEvent::Pointer(sample))? == ProductInputControl::Yield;
    }
    if yield_requested {
        return Ok(ProductInputControl::Yield);
    }
    service_product_ingress(ingress, interact)
}

/// Deliver at most the exact admitted keyboard queue capacity. If any keyboard
/// work ran, return immediately so the caller can re-check the physical source;
/// face/export service runs only from an already-idle ingress turn.
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

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
