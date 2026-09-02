//! Finite application events and their bounded admission queue.

use super::{
    application_view::{decode_event_kind, Cursor},
    ApplicationEventKind, ApplicationView, ApplicationViewRefusal, APPLICATION_VIEW_VERSION,
    MAX_APPLICATION_ACTION_ID_BYTES, MAX_APPLICATION_EVENT_BYTES,
    MAX_APPLICATION_EVENT_ENCODED_BYTES, MAX_APPLICATION_EVENT_QUEUE,
};
use alloc::collections::VecDeque;
use alloc::{string::String, vec::Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationEvent {
    pub revision: u32,
    pub action: String,
    pub kind: ApplicationEventKind,
    pub value: Vec<u8>,
}

impl ApplicationEvent {
    pub fn validate(&self, view: &ApplicationView) -> Result<(), ApplicationViewRefusal> {
        if self.revision != view.revision {
            return Err(ApplicationViewRefusal::StaleRevision);
        }
        if self.value.len() > MAX_APPLICATION_EVENT_BYTES {
            return Err(ApplicationViewRefusal::EventTooLarge);
        }
        let action = view
            .actions
            .iter()
            .find(|candidate| candidate.id == self.action)
            .ok_or(ApplicationViewRefusal::UnknownAction)?;
        if action.event != self.kind {
            return Err(ApplicationViewRefusal::UnknownAction);
        }
        Ok(())
    }

    pub fn encode(&self, view: &ApplicationView) -> Result<Vec<u8>, ApplicationViewRefusal> {
        self.validate(view)?;
        let mut encoded = Vec::with_capacity(9 + self.action.len() + self.value.len());
        encoded.push(APPLICATION_VIEW_VERSION);
        encoded.extend_from_slice(&self.revision.to_le_bytes());
        encoded.push(self.kind as u8);
        encoded.push(self.action.len() as u8);
        encoded.extend_from_slice(&(self.value.len() as u16).to_le_bytes());
        encoded.extend_from_slice(self.action.as_bytes());
        encoded.extend_from_slice(&self.value);
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8], view: &ApplicationView) -> Result<Self, ApplicationViewRefusal> {
        if encoded.len() > MAX_APPLICATION_EVENT_ENCODED_BYTES {
            return Err(ApplicationViewRefusal::EventTooLarge);
        }
        let mut cursor = Cursor::new(encoded);
        if cursor.byte()? != APPLICATION_VIEW_VERSION {
            return Err(ApplicationViewRefusal::UnsupportedVersion);
        }
        let revision = cursor.u32()?;
        let kind = decode_event_kind(cursor.byte()?)?;
        let action_length = usize::from(cursor.byte()?);
        let value_length = usize::from(cursor.u16()?);
        if action_length == 0 || action_length > MAX_APPLICATION_ACTION_ID_BYTES {
            return Err(ApplicationViewRefusal::ActionIdTooLong);
        }
        if value_length > MAX_APPLICATION_EVENT_BYTES {
            return Err(ApplicationViewRefusal::EventTooLarge);
        }
        let action = cursor.text(action_length)?.into();
        let value = cursor.bytes(value_length)?.to_vec();
        if !cursor.complete() {
            return Err(ApplicationViewRefusal::MalformedEncoding);
        }
        let event = Self {
            revision,
            action,
            kind,
            value,
        };
        event.validate(view)?;
        Ok(event)
    }
}

#[derive(Debug)]
pub struct ApplicationEventQueue {
    capacity: usize,
    queued: VecDeque<ApplicationEvent>,
}

impl ApplicationEventQueue {
    pub fn new(capacity: usize) -> Result<Self, ApplicationViewRefusal> {
        if capacity == 0 || capacity > MAX_APPLICATION_EVENT_QUEUE {
            return Err(ApplicationViewRefusal::QueuePressure);
        }
        Ok(Self {
            capacity,
            queued: VecDeque::with_capacity(capacity),
        })
    }

    pub fn push(
        &mut self,
        event: ApplicationEvent,
        view: &ApplicationView,
    ) -> Result<(), ApplicationViewRefusal> {
        event.validate(view)?;
        if self.queued.len() == self.capacity {
            return Err(ApplicationViewRefusal::QueuePressure);
        }
        self.queued.push_back(event);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<ApplicationEvent> {
        self.queued.pop_front()
    }
}
