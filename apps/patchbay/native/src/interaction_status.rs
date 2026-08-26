//! One finite renderer-local channel for interaction and gesture feedback.

use std::time::{Duration, Instant};

#[cfg(test)]
pub const MAX_STATUS_ENTRIES: usize = 1;
pub const MAX_STATUS_TEXT_BYTES: usize = 192;
pub const MAX_STATUS_LIFETIME: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionStatusLevel {
    Success,
    Information,
    Refusal,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionStatusCode {
    Selection,
    Gesture,
    Completed,
    Cancelled,
    Refused,
    PlatformFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionStatus {
    pub sequence: u64,
    pub level: InteractionStatusLevel,
    pub code: InteractionStatusCode,
    pub text: String,
    expires_at: Instant,
}

#[derive(Debug, Default)]
pub struct InteractionStatusChannel {
    current: Option<InteractionStatus>,
    next_sequence: u64,
}

impl InteractionStatusChannel {
    pub fn publish(
        &mut self,
        level: InteractionStatusLevel,
        code: InteractionStatusCode,
        text: impl Into<String>,
    ) {
        self.publish_at(level, code, text, Instant::now());
    }

    fn publish_at(
        &mut self,
        level: InteractionStatusLevel,
        code: InteractionStatusCode,
        text: impl Into<String>,
        now: Instant,
    ) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.current = Some(InteractionStatus {
            sequence,
            level,
            code,
            text: bounded_text(text.into()),
            expires_at: now.checked_add(MAX_STATUS_LIFETIME).unwrap_or(now),
        });
    }

    pub fn current(&mut self) -> Option<&InteractionStatus> {
        self.current_at(Instant::now())
    }

    fn current_at(&mut self, now: Instant) -> Option<&InteractionStatus> {
        if self
            .current
            .as_ref()
            .is_some_and(|status| now >= status.expires_at)
        {
            self.current = None;
        }
        self.current.as_ref()
    }

    pub fn expire_due(&mut self) -> bool {
        let was_present = self.current.is_some();
        let _ = self.current();
        was_present && self.current.is_none()
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.current.as_ref().map(|status| status.expires_at)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        usize::from(self.current.is_some())
    }
}

fn bounded_text(mut text: String) -> String {
    if text.len() <= MAX_STATUS_TEXT_BYTES {
        return text;
    }
    let mut end = MAX_STATUS_TEXT_BYTES.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("...");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_replaces_bounds_and_expires_one_presentation_value() {
        let start = Instant::now();
        let mut channel = InteractionStatusChannel::default();
        channel.publish_at(
            InteractionStatusLevel::Information,
            InteractionStatusCode::Gesture,
            "é".repeat(MAX_STATUS_TEXT_BYTES),
            start,
        );
        assert_eq!(channel.len(), MAX_STATUS_ENTRIES);
        assert!(channel.current_at(start).unwrap().text.len() <= MAX_STATUS_TEXT_BYTES);
        channel.publish_at(
            InteractionStatusLevel::Refusal,
            InteractionStatusCode::Refused,
            "second",
            start,
        );
        assert_eq!(channel.len(), 1);
        assert_eq!(channel.current_at(start).unwrap().text, "second");
        assert!(channel.current_at(start + MAX_STATUS_LIFETIME).is_none());
    }
}
