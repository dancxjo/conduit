//! Bounded kernel-owned merge for the three exact R1 deliberate-input Gears.

use conduit_core::{GearId, PlanFragment};
use conduit_kernel::static_merge::{
    FixedStaticMerge, StaticMergeError, StaticMergeEvent, StaticMergeSource,
};
use conduit_kernel::{NodeId, PortId, ValueRef};
use conduit_runtime::lowering::LoweredPlanFragment;
use conduit_signal::Signal;

pub const R1_CONTROL_PEERS: usize = 3;
pub const R1_CONTROL_MAXIMUM_EVENTS: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R1ControlPeer {
    Terminal,
    BrowserA,
    BrowserB,
}

impl R1ControlPeer {
    const fn index(self) -> usize {
        match self {
            Self::Terminal => 0,
            Self::BrowserA => 1,
            Self::BrowserB => 2,
        }
    }

    const fn gear(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::BrowserA => "browser-a",
            Self::BrowserB => "browser-b",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R1InputEvent {
    pub peer: R1ControlPeer,
    pub peer_sequence: u64,
    pub level: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R1MergedInput {
    pub input: R1InputEvent,
    pub signal: Signal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R1ControlError {
    InvalidPlan,
    WrongPeerSequence,
    DuplicateLevel,
    CapacityExhausted,
    KernelMerge(StaticMergeError),
}

pub struct R1ControlKernel {
    merge: FixedStaticMerge<R1_CONTROL_PEERS, R1_CONTROL_MAXIMUM_EVENTS>,
    sources: [StaticMergeSource; R1_CONTROL_PEERS],
    next_peer_sequence: [u64; R1_CONTROL_PEERS],
    current_level: [bool; R1_CONTROL_PEERS],
    pending: [Option<R1InputEvent>; R1_CONTROL_MAXIMUM_EVENTS],
    next_sequence: u64,
}

impl R1ControlKernel {
    pub fn from_lowered_plan(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, R1ControlError> {
        let mut sources = [StaticMergeSource {
            node: NodeId(u16::MAX),
            port: PortId(u16::MAX),
        }; R1_CONTROL_PEERS];
        for peer in [
            R1ControlPeer::Terminal,
            R1ControlPeer::BrowserA,
            R1ControlPeer::BrowserB,
        ] {
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.gear_id == GearId::from(peer.gear()))
                .ok_or(R1ControlError::InvalidPlan)?;
            if placement.kind_id.as_str() != conduit_signal::LEVEL_INPUT_KIND {
                return Err(R1ControlError::InvalidPlan);
            }
            let node = lowered
                .nodes
                .iter()
                .find(|node| node.placement_id == placement.placement_id)
                .ok_or(R1ControlError::InvalidPlan)?;
            let output = node.outputs.as_slice();
            let [output] = output else {
                return Err(R1ControlError::InvalidPlan);
            };
            sources[peer.index()] = StaticMergeSource {
                node: node.node,
                port: output.port,
            };
        }
        let merge = FixedStaticMerge::new(sources).map_err(R1ControlError::KernelMerge)?;
        Ok(Self {
            merge,
            sources,
            next_peer_sequence: [0; R1_CONTROL_PEERS],
            current_level: [false; R1_CONTROL_PEERS],
            pending: [None; R1_CONTROL_MAXIMUM_EVENTS],
            next_sequence: 0,
        })
    }

    pub fn offer(&mut self, input: R1InputEvent) -> Result<(), R1ControlError> {
        let peer = input.peer.index();
        if input.peer_sequence != self.next_peer_sequence[peer] {
            return Err(R1ControlError::WrongPeerSequence);
        }
        if input.level == self.current_level[peer] {
            return Err(R1ControlError::DuplicateLevel);
        }
        let slot = usize::try_from(self.next_sequence)
            .ok()
            .filter(|slot| *slot < R1_CONTROL_MAXIMUM_EVENTS)
            .ok_or(R1ControlError::CapacityExhausted)?;
        let reference = ValueRef {
            slot: slot as u16,
            generation: 1,
            byte_len: conduit_signal::SIGNAL_ENCODED_LEN,
        };
        self.merge
            .offer(StaticMergeEvent {
                sequence: self.next_sequence,
                source: self.sources[peer],
                value: reference,
            })
            .map_err(R1ControlError::KernelMerge)?;
        self.pending[slot] = Some(input);
        self.next_peer_sequence[peer] += 1;
        self.current_level[peer] = input.level;
        self.next_sequence += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<R1MergedInput> {
        let merged = self.merge.pop()?;
        let input = self.pending[usize::from(merged.value.slot)].take()?;
        Some(R1MergedInput {
            input,
            signal: Signal {
                sequence: merged.sequence,
                level: input.level,
            },
        })
    }

    pub fn pending(&self) -> usize {
        self.merge.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_system_continuity::{exact_r1_control_plan, R1SignalRouteSet};

    fn kernel() -> R1ControlKernel {
        let exact = exact_r1_control_plan(
            conduit_core::BootId::from("r1/pico-test-boot"),
            R1SignalRouteSet::WebSocketThenUsb,
        )
        .unwrap();
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .unwrap();
        let lowered = conduit_runtime::lowering::lower_plan_fragment(fragment).unwrap();
        R1ControlKernel::from_lowered_plan(fragment, &lowered).unwrap()
    }

    #[test]
    fn every_peer_independently_drives_keydown_on_and_keyup_off() {
        let mut kernel = kernel();
        for peer in [
            R1ControlPeer::Terminal,
            R1ControlPeer::BrowserA,
            R1ControlPeer::BrowserB,
        ] {
            kernel
                .offer(R1InputEvent {
                    peer,
                    peer_sequence: 0,
                    level: true,
                })
                .unwrap();
            kernel
                .offer(R1InputEvent {
                    peer,
                    peer_sequence: 1,
                    level: false,
                })
                .unwrap();
        }
        for sequence in 0..6 {
            let merged = kernel.pop().unwrap();
            assert_eq!(merged.signal.sequence, sequence);
            assert_eq!(merged.signal.level, sequence.is_multiple_of(2));
        }
        assert_eq!(kernel.pending(), 0);
    }

    #[test]
    fn duplicate_level_and_peer_sequence_fail_without_mutating_merge() {
        let mut kernel = kernel();
        let first = R1InputEvent {
            peer: R1ControlPeer::BrowserA,
            peer_sequence: 0,
            level: true,
        };
        kernel.offer(first).unwrap();
        assert_eq!(kernel.offer(first), Err(R1ControlError::WrongPeerSequence));
        assert_eq!(
            kernel.offer(R1InputEvent {
                peer: R1ControlPeer::BrowserA,
                peer_sequence: 1,
                level: true,
            }),
            Err(R1ControlError::DuplicateLevel)
        );
        assert_eq!(kernel.pending(), 1);
    }
}
