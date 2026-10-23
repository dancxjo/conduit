//! Scoped route intent, consumed by its existing composition, never a renderer
//! state machine. There is no additional render, guest entry, or retry here.
use super::*;
use crate::protected_compositor::lifecycle::sign::{self, Checkpoint, Position};

impl NativeCompositor {
    pub(crate) fn begin_lifecycle_checkpoint(&self, checkpoint: Checkpoint) -> bool {
        if let Some(previous) = self.lifecycle_checkpoint.take() {
            self.missing_lifecycle_ack(previous);
        }
        let selected = self
            .owner()
            .ok()
            .and_then(|owner| owner.try_borrow_mut().ok())
            .map(|mut owner| owner.select_lifecycle_checkpoint(checkpoint));
        match selected {
            Some(true) => {
                self.lifecycle_checkpoint.set(Some(checkpoint));
                true
            }
            Some(false) => false,
            None => {
                self.missing_lifecycle_ack(checkpoint);
                false
            }
        }
    }

    /// Called after the existing compose interval. A successful FrameIdle has
    /// no copy ACK; it emits incomplete evidence and cannot borrow an older ACK.
    pub(super) fn complete_lifecycle_observation(&self, succeeded: bool) {
        let Some(checkpoint) = self.lifecycle_checkpoint.take() else {
            return;
        };
        let ack = succeeded.then(|| self.lifecycle_copy_ack()).flatten();
        if let Some(ack) = ack {
            if let Some(owner) = self.owner().ok().and_then(|owner| owner.try_borrow().ok()) {
                owner.emit_selected_lifecycle(checkpoint, Some(ack));
                return;
            }
        }
        self.missing_lifecycle_ack(checkpoint);
    }

    /// The route wrapper closes intent even when staging failed before compose.
    pub(crate) fn finish_lifecycle_checkpoint(
        &self,
        checkpoint: Checkpoint,
        selected: bool,
        succeeded: bool,
    ) {
        if let Some(pending) = self.lifecycle_checkpoint.take() {
            self.missing_lifecycle_ack(pending);
        } else if selected && !succeeded {
            self.missing_lifecycle_ack(checkpoint);
        }
    }

    fn missing_lifecycle_ack(&self, checkpoint: Checkpoint) {
        if let Some(owner) = self.owner().ok().and_then(|owner| owner.try_borrow().ok()) {
            owner.emit_missing_lifecycle_ack(checkpoint);
        } else {
            // No invented scope: an unbound explicit incomplete marker is never
            // accepted as product evidence and does not alter the route result.
            let mut buffer = [0; sign::CAPACITY];
            let encoded = sign::encode_missing_ack(
                Position {
                    preparation: 0,
                    checkpoint: checkpoint.ordinal(),
                    part: 0,
                    parts: 6,
                    instance: 0,
                    namespace: None,
                },
                &mut buffer,
            );
            crate::arch::early_write(&buffer[..encoded.length]);
        }
    }
}
