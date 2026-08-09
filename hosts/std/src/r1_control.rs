//! Bounded kernel-owned merge for the three exact R1 deliberate-input Gears.

use conduit_core::{GearId, PlanFragment};
use conduit_kernel::static_merge::{
    FixedStaticMerge, StaticMergeError, StaticMergeEvent, StaticMergeSource,
};
use conduit_kernel::{NodeId, PortId, ValueRef};
use conduit_runtime::lowering::LoweredPlanFragment;
use conduit_signal::Signal;
use std::io::{BufRead, Write};

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

pub fn run_live_three_peer_input(bind: &str) -> Result<(), String> {
    let exact = conduit_system_continuity::exact_r1_control_plan(
        conduit_core::BootId::from("r1/live-input-pico-boot"),
        conduit_system_continuity::R1SignalRouteSet::WebSocketThenUsb,
    )?;
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
        .ok_or_else(|| "R1 control source fragment missing".to_string())?;
    let lowered = conduit_runtime::lowering::lower_plan_fragment(fragment)
        .map_err(|error| format!("R1 control lowering: {error:?}"))?;
    let mut kernel = R1ControlKernel::from_lowered_plan(fragment, &lowered)
        .map_err(|error| format!("R1 control kernel: {error:?}"))?;
    let address = bind
        .parse()
        .map_err(|error| format!("invalid R1 input bind address: {error}"))?;
    let mut listener = crate::external_websocket::ExternalWebSocketListener::bind(address, 2, 10)
        .map_err(|error| format!("R1 browser input listener: {error:?}"))?;
    println!(
        "r1-three-peer-input-ready address={} plan={} input_events=6 physical_led_claim=false",
        listener.local_addr().map_err(debug_error)?,
        exact.plan.plan_id.as_str(),
    );
    std::io::stdout().flush().map_err(debug_error)?;

    let browser_a = listener.accept_peer().map_err(debug_error)?;
    let browser_b = listener.accept_peer().map_err(debug_error)?;
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    for (peer_sequence, expected) in [(0, "down"), (1, "up")] {
        let line = lines
            .next()
            .ok_or_else(|| format!("terminal input ended before {expected}"))?
            .map_err(debug_error)?;
        if line.trim() != expected {
            return Err(format!("terminal input expected '{expected}'"));
        }
        merge_and_record(
            &mut kernel,
            R1InputEvent {
                peer: R1ControlPeer::Terminal,
                peer_sequence,
                level: expected == "down",
            },
        )?;
    }

    for (socket, peer) in [
        (browser_a, R1ControlPeer::BrowserA),
        (browser_b, R1ControlPeer::BrowserB),
    ] {
        for peer_sequence in 0..2 {
            let mut frame = [0_u8; 10];
            let length = listener
                .receive_binary(socket, &mut frame)
                .map_err(debug_error)?;
            let input = decode_browser_event(&frame[..length], peer)?;
            if input.peer_sequence != peer_sequence {
                return Err(format!("browser peer sequence expected {peer_sequence}"));
            }
            let merged = merge_and_record(&mut kernel, input)?;
            let encoded = conduit_signal::encode_signal_fixed(&merged.signal);
            listener
                .send_binary(socket, &encoded)
                .map_err(debug_error)?;
        }
        listener.disconnect(socket).map_err(debug_error)?;
    }
    if kernel.pending() != 0 {
        return Err("R1 input merge retained events at completion".into());
    }
    println!(
        "r1-three-peer-input-complete plan={} input_events=6 physical_led_claim=false",
        exact.plan.plan_id.as_str()
    );
    Ok(())
}

fn decode_browser_event(bytes: &[u8], expected: R1ControlPeer) -> Result<R1InputEvent, String> {
    let [peer, sequence @ .., level] = bytes else {
        return Err("browser input frame must be exactly ten bytes".into());
    };
    let actual = match *peer {
        1 => R1ControlPeer::BrowserA,
        2 => R1ControlPeer::BrowserB,
        _ => return Err("browser input frame has an unknown peer".into()),
    };
    if actual != expected || (*level != 0 && *level != 1) {
        return Err("browser input frame peer or level mismatch".into());
    }
    let sequence: [u8; 8] = sequence
        .try_into()
        .map_err(|_| "browser input frame sequence width")?;
    Ok(R1InputEvent {
        peer: actual,
        peer_sequence: u64::from_le_bytes(sequence),
        level: *level == 1,
    })
}

fn merge_and_record(
    kernel: &mut R1ControlKernel,
    input: R1InputEvent,
) -> Result<R1MergedInput, String> {
    kernel
        .offer(input)
        .map_err(|error| format!("R1 input admission: {error:?}"))?;
    let merged = kernel
        .pop()
        .ok_or_else(|| "R1 kernel merge lost admitted input".to_string())?;
    println!(
        "{}",
        serde_json::json!({
            "schema": "conduit-r1/input-sign@1",
            "peer": match merged.input.peer {
                R1ControlPeer::Terminal => "terminal",
                R1ControlPeer::BrowserA => "browser-a",
                R1ControlPeer::BrowserB => "browser-b",
            },
            "peer_sequence": merged.input.peer_sequence,
            "input": if merged.input.level { "keydown" } else { "keyup" },
            "requested_level": merged.input.level,
            "merged_sequence": merged.signal.sequence,
            "physical_led_result": null,
        })
    );
    Ok(merged)
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
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

    #[test]
    fn browser_frames_bind_exact_peer_width_sequence_and_level() {
        let mut valid = [0_u8; 10];
        valid[0] = 1;
        valid[1..9].copy_from_slice(&7_u64.to_le_bytes());
        valid[9] = 1;
        assert_eq!(
            decode_browser_event(&valid, R1ControlPeer::BrowserA),
            Ok(R1InputEvent {
                peer: R1ControlPeer::BrowserA,
                peer_sequence: 7,
                level: true,
            })
        );
        assert!(decode_browser_event(&valid[..9], R1ControlPeer::BrowserA).is_err());
        assert!(decode_browser_event(&valid, R1ControlPeer::BrowserB).is_err());
        valid[0] = 9;
        assert!(decode_browser_event(&valid, R1ControlPeer::BrowserA).is_err());
        valid[0] = 1;
        valid[9] = 2;
        assert!(decode_browser_event(&valid, R1ControlPeer::BrowserA).is_err());
    }
}
