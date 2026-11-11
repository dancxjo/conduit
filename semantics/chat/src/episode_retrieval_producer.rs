//! Prepare a temporal window and immutable capture for two ordinary Cords.
use crate::episode_retrieval_packet::*;
use crate::PreparedEpisodeRetrievalWindow;
use alloc::vec::Vec;
use conduit_core::validate_canonical_structured_value;

pub struct PreparedEpisodeRetrievalProducer {
    body: Option<conduit_body::BodyConversationContext>,
    capture_type: Vec<u8>,
    clock_type: Vec<u8>,
    memory_type: Vec<u8>,
    published: bool,
    capture: Vec<u8>,
    packet: PreparedEpisodeRetrievalPacket,
    window: PreparedEpisodeRetrievalWindow,
    begun: Option<u64>,
    staged: Option<u64>,
}
impl Default for PreparedEpisodeRetrievalProducer {
    fn default() -> Self {
        Self::prepare()
    }
}
impl PreparedEpisodeRetrievalProducer {
    pub fn prepare() -> Self {
        Self {
            body: None,
            capture_type: episode_retrieval_capture_type().canonical_bytes().unwrap(),
            clock_type: episode_retrieval_clock_class_type()
                .canonical_bytes()
                .unwrap(),
            memory_type: crate::episode_memory_state_type()
                .canonical_bytes()
                .unwrap(),
            published: false,
            capture: Vec::with_capacity(MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES),
            packet: PreparedEpisodeRetrievalPacket::prepare(),
            window: PreparedEpisodeRetrievalWindow::prepare(),
            begun: None,
            staged: None,
        }
    }
    pub fn has_context(&self) -> bool {
        self.body.is_some()
    }
    /// Uses the same canonical Body context admission as the existing composer.
    pub fn admit_context(&mut self, bytes: &[u8]) -> Result<(), EpisodeRetrievalPacketRefusal> {
        use EpisodeRetrievalPacketRefusal as R;
        let next = crate::decode_body_conversation_context(bytes).map_err(|_| R::Context)?;
        if self.body.as_ref().is_some_and(|old| {
            old.body_id != next.body_id
                || old.wake_id != next.wake_id
                || old.wake_sequence != next.wake_sequence
        }) {
            return Err(R::Context);
        }
        self.body = Some(next);
        Ok(())
    }
    pub fn check_memory(&self, bytes: &[u8]) -> Result<u64, EpisodeRetrievalPacketRefusal> {
        use EpisodeRetrievalPacketRefusal as R;
        if bytes.len() > crate::MAXIMUM_EPISODE_MEMORY_BYTES {
            return Err(R::ByteBound);
        }
        let memory = validate_canonical_structured_value(bytes).map_err(|_| R::Malformed)?;
        if memory.type_bytes() != self.memory_type {
            return Err(R::Malformed);
        }
        let completed = number(field(memory, "completed")?)?;
        let owner = field(memory, "owner")?;
        match owner.variant_payload("bound").map_err(|_| R::Malformed)? {
            Some(owner) => {
                let body = self.body.as_ref().ok_or(R::Context)?;
                if completed == 0
                    || text(field(owner, "body_id")?)? != body.body_id.as_str()
                    || text(field(owner, "wake_id")?)? != body.wake_id.as_str()
                    || number(field(owner, "wake_sequence")?)? != body.wake_sequence
                {
                    return Err(R::Context);
                }
            }
            None if completed != 0 => return Err(R::Context),
            None => {}
        }
        if self.begun.is_some_and(|begun| begun != completed) {
            return Err(R::OutOfOrder);
        }
        Ok(completed)
    }
    pub fn stage(
        &mut self,
        experience: &[u8],
        memory: &[u8],
        intent: &[u8],
        policy: &[u8],
        clock: &[u8],
    ) -> Result<(), EpisodeRetrievalPacketRefusal> {
        use EpisodeRetrievalPacketRefusal as R;
        self.staged = None;
        self.published = false;
        self.capture.clear();
        let completed = self.check_memory(memory)?;
        let body = self.body.as_ref().ok_or(R::Context)?;
        let clock = validate_canonical_structured_value(clock).map_err(|_| R::Malformed)?;
        if clock.type_bytes() != self.clock_type {
            return Err(R::Malformed);
        }
        put(&mut self.capture, &self.capture_type)?;
        put(&mut self.capture, &[2])?;
        put(&mut self.capture, &9u32.to_le_bytes())?;
        for (key, value) in [
            ("body_id", body.body_id.as_str().as_bytes()),
            ("clock_class", clock.value_node()),
            ("context_policy", policy),
            ("display_name", body.display_name.as_bytes()),
            ("experience", experience),
            ("intent", intent),
            ("memory", memory),
            ("wake_id", body.wake_id.as_str().as_bytes()),
            ("wake_sequence", &body.wake_sequence.to_le_bytes()),
        ] {
            name(&mut self.capture, key)?;
            if key == "clock_class" {
                put(&mut self.capture, value)?;
            } else {
                scalar(&mut self.capture, value)?;
            }
        }
        let (snapshot, _) = self.packet.decode(&self.capture)?;
        self.window.build(&snapshot).map_err(R::Window)?;
        self.staged = Some(completed.checked_add(1).ok_or(R::OutOfOrder)?);
        self.published = true;
        Ok(())
    }
    pub fn window(&self) -> &[u8] {
        if self.published {
            self.window.output()
        } else {
            &[]
        }
    }
    pub fn capture(&self) -> &[u8] {
        if self.published {
            &self.capture
        } else {
            &[]
        }
    }
    pub fn commit(&mut self) {
        if let Some(staged) = self.staged.take() {
            self.begun = Some(staged);
        }
    }
    pub fn cancel(&mut self) {
        self.staged = None;
        self.published = false;
        self.capture.clear();
    }
}
