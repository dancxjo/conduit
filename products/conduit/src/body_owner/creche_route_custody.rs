//! Receipt custody for one scoped Crèche route, not a renderer or input owner.
//! Only the trusted Body owner may record a return, AFTER its existing browser
//! or native Mask route validated the actual typed occurrence. This is not a
//! raw local-control entrance and structural Available alone is never evidence.
use conduit::creche_encounter::CrecheMaskShowCustody;
use conduit_face::{Face, FaceInteraction, MaskShow};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

#[derive(Clone)]
pub(super) struct CrecheRouteCustody {
    state: Arc<Mutex<State>>,
}
struct Receipt {
    face_id: String,
    revision: u64,
    show: MaskShow,
    input: FaceInteraction,
}
#[derive(Default)]
struct State {
    closed: bool,
    receipt: Option<Receipt>,
}
impl CrecheRouteCustody {
    /// Clone this handle into `Box<dyn CrecheMaskShowCustody>` for the adapter;
    /// the service retains the recording handle and actual Mask effect owners.
    pub(super) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
        }
    }
    fn lock(&self) -> Result<MutexGuard<'_, State>, String> {
        match self.state.try_lock() {
            Ok(state) => Ok(state),
            Err(TryLockError::WouldBlock) => Err("Crèche route receipt is busy".into()),
            Err(TryLockError::Poisoned(_)) => Err("Crèche route receipt is poisoned".into()),
        }
    }
    /// Trusted caller origin is essential: call only after the existing Owner
    /// `validate_browser_mask_interaction` or `validate_native_mask_interaction`
    /// succeeded. This recheck correlates structure; it cannot observe a device.
    /// Keep a queued command's receipt until the validated current Face changes
    /// revision. An exact retry is idempotent and never consumes the receipt.
    pub(super) fn record_validated_return(
        &self,
        face: &Face,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<(), String> {
        input
            .validate_against(face, show)
            .map_err(|error| format!("Crèche route return structure: {error:?}"))?;
        let mut state = self.lock()?;
        if state.closed {
            return Err("Crèche route receipt is closed".into());
        }
        if let Some(receipt) = &state.receipt {
            if receipt.revision == face.revision {
                return if receipt.face_id == face.identity.as_str()
                    && receipt.show == *show
                    && receipt.input == *input
                {
                    Ok(())
                } else {
                    Err("Crèche route has a different pending return at this Face revision".into())
                };
            }
        }
        state.receipt = Some(Receipt {
            face_id: face.identity.as_str().into(),
            revision: face.revision,
            show: show.clone(),
            input: input.clone(),
        });
        Ok(())
    }
}
impl CrecheMaskShowCustody for CrecheRouteCustody {
    fn validate_acknowledged(&mut self, face: &Face, show: &MaskShow) -> Result<(), String> {
        let state = self.lock()?;
        if state.closed {
            return Err("Crèche route receipt is closed".into());
        }
        let receipt = state
            .receipt
            .as_ref()
            .ok_or("Crèche route has no validated return receipt")?;
        if receipt.face_id != face.identity.as_str()
            || receipt.revision != face.revision
            || receipt.show != *show
        {
            return Err("Crèche route Show differs from the recorded return".into());
        }
        receipt
            .input
            .validate_against(face, show)
            .map_err(|error| format!("Crèche recorded return structure: {error:?}"))
    }
    fn validate_returned_interaction(
        &mut self,
        face: &Face,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<(), String> {
        let state = self.lock()?;
        if state.closed {
            return Err("Crèche route receipt is closed".into());
        }
        let receipt = state
            .receipt
            .as_ref()
            .ok_or("Crèche route has no validated return receipt")?;
        if receipt.face_id != face.identity.as_str()
            || receipt.revision != face.revision
            || receipt.show != *show
            || receipt.input != *input
        {
            return Err("Crèche interaction differs from the recorded typed return".into());
        }
        input
            .validate_against(face, show)
            .map_err(|error| format!("Crèche recorded interaction structure: {error:?}"))
    }
    /// Close only this scoped receipt. The service independently owns the Mask
    /// workers/effects; this success makes NO claim that those effects drained.
    fn retire_and_drain(&mut self) -> Result<(), String> {
        let mut state = self.lock()?;
        state.closed = true;
        state.receipt = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{
        FaceAction, FaceActionArgument, FaceActionAvailability, FaceDisclosureLevel,
        FaceInteractionArgument, UTF8_TEXT_VALUE_KIND,
    };

    fn face(revision: u64) -> Face {
        let plot = crate::mask_test_common::checked_renderer_plot();
        let host = crate::mask_test_common::host(
            "receipt-host",
            "receipt-boot",
            "renderer",
            "renderer@1",
            "renderer-artifact@1",
            "face/base/test@1",
            crate::mask_test_common::DOM_RESOURCE,
        );
        let plan = crate::mask_test_common::plan_for(&plot, host);
        let base = crate::mask_test_common::face(&plot, &plan);
        Face::new_with_semantics(
            revision,
            base.basis,
            base.subjects,
            base.relationships,
            base.properties,
            base.text,
            vec![FaceAction {
                identity: "fixture/edit".into(),
                intent: "fixture/edit@1".into(),
                target: "patchbay/plot".into(),
                name: "Edit".into(),
                arguments: vec![
                    FaceActionArgument::text("value".into(), "Value".into(), 1, 32).unwrap(),
                ],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            }],
            base.disclosures,
        )
        .unwrap()
    }
    fn input(face: &Face, show: &MaskShow, value: &str, sequence: u64) -> FaceInteraction {
        FaceInteraction::new(
            face,
            show,
            "fixture/edit",
            "patchbay/plot",
            vec![FaceInteractionArgument {
                name: "value".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: value.as_bytes().to_vec(),
            }],
            sequence,
        )
        .unwrap()
    }
    #[test]
    fn unrecorded_available_show_refuses_and_recorded_return_can_be_rechecked() {
        let face = face(1);
        // Existing software fixture models ACK only; recording below models the
        // trusted owner after real route validation, not a physical proof.
        let show = crate::mask_test_common::available_mask_show(&face);
        let input = input(&face, &show, "first", 1);
        let handle = CrecheRouteCustody::new();
        let mut adapter: Box<dyn CrecheMaskShowCustody> = Box::new(handle.clone());
        assert!(adapter.validate_acknowledged(&face, &show).is_err());
        assert!(adapter
            .validate_returned_interaction(&face, &show, &input)
            .is_err());
        handle
            .record_validated_return(&face, &show, &input)
            .unwrap();
        handle
            .record_validated_return(&face, &show, &input)
            .unwrap();
        for _ in 0..2 {
            adapter.validate_acknowledged(&face, &show).unwrap();
            adapter
                .validate_returned_interaction(&face, &show, &input)
                .unwrap();
        }
        let mut changed = input.clone();
        changed.arguments[0].value = b"other".to_vec();
        assert!(adapter
            .validate_returned_interaction(&face, &show, &changed)
            .is_err());
    }
    #[test]
    fn different_pending_input_cannot_replace_receipt_until_face_revision_changes() {
        let face = face(1);
        let show = crate::mask_test_common::available_mask_show(&face);
        let first = input(&face, &show, "first", 1);
        let second = input(&face, &show, "second", 2);
        let handle = CrecheRouteCustody::new();
        let mut adapter = handle.clone();
        handle
            .record_validated_return(&face, &show, &first)
            .unwrap();
        assert!(handle
            .record_validated_return(&face, &show, &second)
            .is_err());
        adapter
            .validate_returned_interaction(&face, &show, &first)
            .unwrap();
        let next = self::face(2);
        let next_show = crate::mask_test_common::available_mask_show(&next);
        let next_input = input(&next, &next_show, "second", 2);
        handle
            .record_validated_return(&next, &next_show, &next_input)
            .unwrap();
        adapter
            .validate_returned_interaction(&next, &next_show, &next_input)
            .unwrap();
        assert!(adapter.validate_acknowledged(&face, &show).is_err());
    }
    #[test]
    fn contention_and_retirement_refuse_late_returns_for_all_clones() {
        let face = face(1);
        let show = crate::mask_test_common::available_mask_show(&face);
        let input = input(&face, &show, "first", 1);
        let handle = CrecheRouteCustody::new();
        let mut adapter = handle.clone();
        handle
            .record_validated_return(&face, &show, &input)
            .unwrap();
        let guard = handle.state.try_lock().unwrap();
        assert!(adapter
            .validate_acknowledged(&face, &show)
            .unwrap_err()
            .contains("busy"));
        assert!(handle
            .record_validated_return(&face, &show, &input)
            .unwrap_err()
            .contains("busy"));
        assert!(adapter.retire_and_drain().unwrap_err().contains("busy"));
        drop(guard);
        adapter.retire_and_drain().unwrap();
        adapter.retire_and_drain().unwrap();
        assert!(handle
            .record_validated_return(&face, &show, &input)
            .is_err());
        assert!(adapter.validate_acknowledged(&face, &show).is_err());
        assert!(adapter
            .validate_returned_interaction(&face, &show, &input)
            .is_err());
        assert!(handle.state.try_lock().unwrap().receipt.is_none());
    }
    #[test]
    fn structurally_changed_show_never_replaces_validated_receipt() {
        let face = face(1);
        let show = crate::mask_test_common::available_mask_show(&face);
        let input = input(&face, &show, "first", 1);
        let handle = CrecheRouteCustody::new();
        let mut adapter = handle.clone();
        handle
            .record_validated_return(&face, &show, &input)
            .unwrap();
        let mut invalid = show.clone();
        invalid.face_revision += 1;
        assert!(handle
            .record_validated_return(&face, &invalid, &input)
            .is_err());
        assert!(adapter.validate_acknowledged(&face, &invalid).is_err());
        adapter
            .validate_returned_interaction(&face, &show, &input)
            .unwrap();
    }
}
