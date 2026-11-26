//! Selected raw Full entrance; Legacy frames keep their original admission.
use super::super::full_profile_pool::Lease;
use super::super::full_refusal::FullPreparationRefusal as Refusal;
use super::*;
use std::rc::Rc;

pub(super) fn try_prepare(basis_len: usize, frame_len: usize) -> Option<i32> {
    if !FULL_INTERFACE.with(Cell::get) {
        return None;
    }
    // A prepared Full Host owns the parser slot before discriminator work.
    // Slot/lifecycle refusal here describes that Host preparation context;
    // positively unselected frames below retain the original Legacy path.
    let lease = match Lease::acquire_current() {
        Ok(v) => v,
        Err(error) => return Some(error.status()),
    };
    if let Err(error) = lease.admit(super::super::full_input::parser_workspace_bytes()) {
        return Some(error.status());
    }
    let decoded = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let frame = &input[basis_len..basis_len + frame_len];
        let selected = match super::super::full_input::selected(frame) {
            Ok(v) => v,
            Err(error) => {
                input[..basis_len + frame_len].fill(0);
                return Err(error.status());
            }
        };
        if !selected {
            if basis_len + frame_len > INPUT_CAPACITY
                || frame_len > conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES + 256
            {
                input[..basis_len + frame_len].fill(0);
                return Err(-1);
            }
            return Ok(None);
        }
        if CURRENT.with(|c| {
            c.borrow()
                .as_ref()
                .is_some_and(|m| m.show.show.lifecycle == ManifestationLifecycle::Prepared)
        }) {
            input[..basis_len + frame_len].fill(0);
            return Err(Refusal::Lifecycle.status());
        }
        if basis_len > 12 * 1024 {
            input[..basis_len + frame_len].fill(0);
            return Err(Refusal::Pressure.status());
        }
        let result = (|| {
            let basis = serde_json::from_slice::<HostBasis>(&input[..basis_len])
                .map_err(|_| Refusal::Malformed.status())?;
            let carrier =
                super::super::full_input::prepare(frame, lease).map_err(Refusal::status)?;
            Ok(Some((basis, carrier)))
        })();
        input[..basis_len + frame_len].fill(0);
        result
    });
    let (basis, mut carrier) = match decoded {
        Ok(Some(v)) => v,
        Ok(None) => return None,
        Err(code) => return Some(code),
    };
    let retirement = match prepare_retirement(&carrier.lease, &mut carrier.input_charge) {
        Ok(v) => v,
        Err(error) => return Some(error.status()),
    };
    let sequence = NEXT_PLAY_SEQUENCE.with(|next| {
        let n = next.get();
        next.set(n.saturating_add(1));
        (n < u64::MAX).then_some(n)
    });
    let Some(sequence) = sequence else {
        return Some(-8);
    };
    let mask = match OwnerBrowserMask::prepare_full(basis, carrier, sequence) {
        Ok(v) => v,
        Err(error) => return Some(error.status()),
    };
    let Some(full) = mask.full.as_ref() else {
        return Some(-5);
    };
    let result = write_full_output(full.view(mask.show.show.lifecycle));
    if result != 0 {
        return Some(result);
    }
    let installed = CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        if let Some(previous) = current.as_mut() {
            if previous.scheduler.cancel().is_err() {
                return false;
            }
            let replaced = if let Some(full) = previous.full.as_mut() {
                full.replaced_show.take()
            } else {
                retirement
            };
            let Some(replaced) = replaced else {
                return false;
            };
            previous.show = replaced;
        }
        *current = Some(mask);
        true
    });
    Some(if installed { 0 } else { -9 })
}

/// A Legacy generic Show's replacement clone is prepared before starting the
/// candidate Play. Configured legacy graphs require an ordinary explicit clear;
/// this profile never guesses their generated-schema workspace.
fn prepare_retirement(lease: &Rc<Lease>, charge: &mut usize) -> Result<Option<MaskShow>, Refusal> {
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(previous) = current.as_ref() else {
            return Ok(None);
        };
        if let Some(full) = previous.full.as_ref() {
            if full.replaced_show.is_none() {
                return Err(Refusal::Lifecycle);
            }
            return Ok(None);
        }
        let codec = conduit_face::PlannedMaskPlot::json_wire_preparation_bytes() + 256 * 1024;
        lease.admit(charge.checked_add(codec).ok_or(Refusal::Pressure)?)?;
        let bytes = super::super::full_refusal::bounded_json(&previous.planned.plan, 256 * 1024)?;
        let model = conduit_face::PlannedMaskPlot::generic_json_plan_preparation_bytes(&bytes)
            .map_err(|_| Refusal::Unsupported)?;
        // Only the old Show/Plan and transition metadata are cloned; its
        // already rendered Face reading is neither rebuilt nor reinterpreted.
        let extra = super::super::full_outputs::replacement_preparation_bytes(model)
            .ok_or(Refusal::Pressure)?;
        *charge = charge
            .checked_add(codec)
            .and_then(|n| n.checked_add(extra))
            .ok_or(Refusal::Pressure)?;
        lease.admit(*charge)?;
        previous
            .show
            .transition(
                ManifestationLifecycle::Replaced,
                SignId::from(format!(
                    "sign/browser-owner-face-replaced/{}",
                    previous.face.revision
                )),
            )
            .map(Some)
            .map_err(|_| Refusal::Preparation)
    })
}
