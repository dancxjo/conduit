//! Bounded merge state for an exact static set of planned input Ports.

use crate::{NodeId, PortId, ValueRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticMergeSource {
    pub node: NodeId,
    pub port: PortId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticMergeEvent {
    pub sequence: u64,
    pub source: StaticMergeSource,
    pub value: ValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticMergeError {
    InvalidCapacity,
    QueueFull,
    UnknownSource,
    DuplicateOrOutOfOrderSequence,
}

/// Fixed-storage many-to-one ordering for a sealed set of source Ports.
pub struct FixedStaticMerge<const SOURCES: usize, const EVENTS: usize> {
    sources: [StaticMergeSource; SOURCES],
    events: [Option<StaticMergeEvent>; EVENTS],
    head: usize,
    len: usize,
    last_sequence: Option<u64>,
}

impl<const SOURCES: usize, const EVENTS: usize> FixedStaticMerge<SOURCES, EVENTS> {
    pub fn new(sources: [StaticMergeSource; SOURCES]) -> Result<Self, StaticMergeError> {
        if SOURCES == 0
            || EVENTS == 0
            || sources.iter().enumerate().any(|(index, source)| {
                sources[..index].contains(source)
                    || source.node == NodeId(u16::MAX)
                    || source.port == PortId(u16::MAX)
            })
        {
            return Err(StaticMergeError::InvalidCapacity);
        }
        Ok(Self {
            sources,
            events: [None; EVENTS],
            head: 0,
            len: 0,
            last_sequence: None,
        })
    }

    pub fn offer(&mut self, event: StaticMergeEvent) -> Result<(), StaticMergeError> {
        if !self.sources.contains(&event.source) {
            return Err(StaticMergeError::UnknownSource);
        }
        if self.len == EVENTS {
            return Err(StaticMergeError::QueueFull);
        }
        if self
            .last_sequence
            .is_some_and(|last| event.sequence <= last)
        {
            return Err(StaticMergeError::DuplicateOrOutOfOrderSequence);
        }
        let index = (self.head + self.len) % EVENTS;
        self.events[index] = Some(event);
        self.len += 1;
        self.last_sequence = Some(event.sequence);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<StaticMergeEvent> {
        let event = self.events[self.head].take()?;
        self.head = (self.head + 1) % EVENTS;
        self.len -= 1;
        Some(event)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
