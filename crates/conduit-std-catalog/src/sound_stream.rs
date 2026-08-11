//! Finite admission accounting shared by portable sound stream contracts.

use crate::{stream_semantics, CancellationDisposition, StreamSemantics};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SoundStreamState {
    Open,
    Draining,
    Cancelled,
    Closed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SoundStreamRefusal {
    UnknownContract,
    ItemTooLarge {
        maximum_bytes: u32,
        actual_bytes: u32,
    },
    Full {
        available_items: u16,
        available_bytes: u32,
        requested_bytes: u32,
    },
    Cancelled,
    Draining,
    Closed,
    InvalidRelease,
}

/// Allocation-free accounting for one exact Plan-admitted stream envelope.
///
/// This object does not schedule or retain values. It proves whether the
/// already-admitted finite storage may accept one value before the producer
/// consumes work or commits output.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SoundStreamAdmission {
    limits: StreamSemantics,
    state: SoundStreamState,
    retained_items: u16,
    retained_bytes: u32,
}

impl SoundStreamAdmission {
    pub fn for_kind(kind: &str) -> Result<Self, SoundStreamRefusal> {
        let limits = stream_semantics(kind).ok_or(SoundStreamRefusal::UnknownContract)?;
        Ok(Self {
            limits,
            state: SoundStreamState::Open,
            retained_items: 0,
            retained_bytes: 0,
        })
    }

    pub const fn state(&self) -> SoundStreamState {
        self.state
    }

    pub const fn retained_items(&self) -> u16 {
        self.retained_items
    }

    pub const fn retained_bytes(&self) -> u32 {
        self.retained_bytes
    }

    pub fn admit(&mut self, value_bytes: u32) -> Result<(), SoundStreamRefusal> {
        match self.state {
            SoundStreamState::Cancelled => return Err(SoundStreamRefusal::Cancelled),
            SoundStreamState::Draining => return Err(SoundStreamRefusal::Draining),
            SoundStreamState::Closed => return Err(SoundStreamRefusal::Closed),
            SoundStreamState::Open => {}
        }
        if value_bytes > self.limits.maximum_queue_bytes {
            return Err(SoundStreamRefusal::ItemTooLarge {
                maximum_bytes: self.limits.maximum_queue_bytes,
                actual_bytes: value_bytes,
            });
        }
        let available_items = self
            .limits
            .maximum_queue_items
            .saturating_sub(self.retained_items);
        let available_bytes = self
            .limits
            .maximum_queue_bytes
            .saturating_sub(self.retained_bytes);
        if available_items == 0 || value_bytes > available_bytes {
            return Err(SoundStreamRefusal::Full {
                available_items,
                available_bytes,
                requested_bytes: value_bytes,
            });
        }
        self.retained_items += 1;
        self.retained_bytes += value_bytes;
        Ok(())
    }

    pub fn release(&mut self, value_bytes: u32) -> Result<(), SoundStreamRefusal> {
        if self.retained_items == 0 || value_bytes > self.retained_bytes {
            return Err(SoundStreamRefusal::InvalidRelease);
        }
        self.retained_items -= 1;
        self.retained_bytes -= value_bytes;
        if self.state == SoundStreamState::Draining && self.retained_items == 0 {
            self.state = SoundStreamState::Cancelled;
        }
        Ok(())
    }

    pub fn request_cancel(&mut self) {
        match self.limits.cancellation {
            CancellationDisposition::CancelAndReleaseFiniteState => {
                self.retained_items = 0;
                self.retained_bytes = 0;
                self.state = SoundStreamState::Cancelled;
            }
            CancellationDisposition::DrainThenComplete => {
                self.state = if self.retained_items == 0 {
                    SoundStreamState::Cancelled
                } else {
                    SoundStreamState::Draining
                };
            }
        }
    }

    pub fn close(&mut self) {
        self.state = SoundStreamState::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AUDIO_PLAY_KIND, MUSIC_PLAY_KIND};

    #[test]
    fn pressure_does_not_consume_or_grow_hidden_storage() {
        let mut stream = SoundStreamAdmission::for_kind(MUSIC_PLAY_KIND).unwrap();
        let limit = stream.limits.maximum_queue_bytes;
        stream.admit(limit).unwrap();
        let before = stream;
        assert!(matches!(
            stream.admit(1),
            Err(SoundStreamRefusal::Full { .. })
        ));
        assert_eq!(stream, before);
        stream.release(limit).unwrap();
        assert_eq!((stream.retained_items(), stream.retained_bytes()), (0, 0));
    }

    #[test]
    fn oversized_full_cancelled_and_closed_remain_distinct() {
        let mut stream = SoundStreamAdmission::for_kind(AUDIO_PLAY_KIND).unwrap();
        let maximum = stream.limits.maximum_queue_bytes;
        assert_eq!(
            stream.admit(maximum + 1),
            Err(SoundStreamRefusal::ItemTooLarge {
                maximum_bytes: maximum,
                actual_bytes: maximum + 1
            })
        );
        stream.admit(maximum).unwrap();
        assert!(matches!(
            stream.admit(1),
            Err(SoundStreamRefusal::Full { .. })
        ));
        stream.request_cancel();
        assert_eq!(stream.state(), SoundStreamState::Draining);
        assert_eq!(stream.admit(1), Err(SoundStreamRefusal::Draining));
        assert_eq!(
            (stream.retained_items(), stream.retained_bytes()),
            (1, maximum)
        );
        stream.release(maximum).unwrap();
        assert_eq!(stream.state(), SoundStreamState::Cancelled);
        assert_eq!((stream.retained_items(), stream.retained_bytes()), (0, 0));

        let mut closed = SoundStreamAdmission::for_kind(AUDIO_PLAY_KIND).unwrap();
        closed.close();
        assert_eq!(closed.admit(1), Err(SoundStreamRefusal::Closed));
    }

    #[test]
    fn musical_cancellation_releases_finite_state_immediately() {
        let mut stream = SoundStreamAdmission::for_kind(MUSIC_PLAY_KIND).unwrap();
        stream.admit(128).unwrap();
        stream.request_cancel();
        assert_eq!(stream.state(), SoundStreamState::Cancelled);
        assert_eq!((stream.retained_items(), stream.retained_bytes()), (0, 0));
    }
}
