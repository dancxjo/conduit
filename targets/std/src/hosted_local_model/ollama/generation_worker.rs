//! One prepared effect worker. It executes only requests dispatched by the
//! kernel; it owns no cadence, Gear queue, retries, or cognitive policy.
use super::*;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
struct Slot {
    input: Vec<u8>,
    output: Vec<u8>,
    placement: usize,
    next_sequence: u64,
    terminal: LocalModelAdapterTerminal,
}
pub(super) struct Worker {
    placements: Vec<PlannedGear>,
    submit: Option<SyncSender<Slot>>,
    completed: Receiver<Slot>,
    slot: Option<Slot>,
    thread: Option<std::thread::JoinHandle<()>>,
    cancelled: bool,
    pub(super) next_sequence: u64,
}
impl Worker {
    pub(super) fn prepare(
        owner: &OllamaLocalModelAdapter,
        placements: &[PlannedGear],
    ) -> Result<Self, String> {
        let placements = placements
            .iter()
            .filter(|p| p.kind_id.as_str() == conduit_ai::LLM_GENERATE_FLOW_KIND)
            .cloned()
            .collect::<Vec<_>>();
        if placements.is_empty() || placements.len() > 8 {
            return Err("generation worker requires 1 to 8 admitted placements".into());
        }
        let bound = |name| {
            placements
                .iter()
                .map(|p| configuration_count(p, name))
                .collect::<Option<Vec<_>>>()
                .map(|v| v.into_iter().max().unwrap_or(0) as usize)
        };
        let input = bound("maximum-input-bytes").ok_or("worker input bound missing")?;
        let output = bound("maximum-output-bytes").ok_or("worker output bound missing")?;
        if input > conduit_ai::MAXIMUM_LLM_INPUT_BYTES as usize
            || output > conduit_ai::MAXIMUM_LLM_OUTPUT_BYTES as usize
        {
            return Err("generation worker envelope exceeds the model profile".into());
        }
        let mut adapter = OllamaLocalModelAdapter {
            offer: owner.offer.clone(),
            endpoint: owner.endpoint.clone(),
            model_name: owner.model_name.clone(),
            next_request_sequence: owner.next_request_sequence,
            active_stream: None,
            generation_worker: None,
        };
        let table = placements.clone();
        let (submit, requests) = sync_channel::<Slot>(1);
        let (results, completed) = sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("conduit-model-effect".into())
            .spawn(move || {
                while let Ok(mut slot) = requests.recv() {
                    adapter.next_request_sequence =
                        adapter.next_request_sequence.max(slot.next_sequence);
                    slot.terminal =
                        adapter.execute(&table[slot.placement], &slot.input, &mut slot.output);
                    slot.next_sequence = adapter.next_request_sequence;
                    if slot.output.len() > output {
                        slot.output.clear();
                        slot.terminal = LocalModelAdapterTerminal::InvalidStructuredResult;
                    }
                    if results.send(slot).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| format!("prepare model effect worker: {e}"))?;
        Ok(Self {
            placements,
            submit: Some(submit),
            completed,
            slot: Some(Slot {
                input: Vec::with_capacity(input),
                output: Vec::with_capacity(output),
                placement: 0,
                next_sequence: owner.next_request_sequence,
                terminal: LocalModelAdapterTerminal::Failed,
            }),
            thread: Some(thread),
            cancelled: false,
            next_sequence: owner.next_request_sequence,
        })
    }
    pub(super) fn begin(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        next_sequence: u64,
    ) -> Option<LocalModelAdapterTerminal> {
        if self.cancelled {
            let _ = self.poll(&mut Vec::new());
        }
        let Some(index) = self.placements.iter().position(|p| p == placement) else {
            return Some(LocalModelAdapterTerminal::Refused);
        };
        let Some(mut slot) = self.slot.take() else {
            return Some(LocalModelAdapterTerminal::Refused);
        };
        if input.len() > slot.input.capacity() {
            self.slot = Some(slot);
            return Some(LocalModelAdapterTerminal::Refused);
        }
        slot.input.clear();
        slot.input.extend_from_slice(input);
        slot.output.clear();
        slot.placement = index;
        slot.next_sequence = next_sequence;
        self.cancelled = false;
        match self.submit.as_ref().unwrap().try_send(slot) {
            Ok(()) => None,
            Err(
                std::sync::mpsc::TrySendError::Full(slot)
                | std::sync::mpsc::TrySendError::Disconnected(slot),
            ) => {
                self.slot = Some(slot);
                Some(LocalModelAdapterTerminal::ProviderLost)
            }
        }
    }
    pub(super) fn poll(&mut self, output: &mut Vec<u8>) -> Option<LocalModelAdapterTerminal> {
        let mut slot = match self.completed.try_recv() {
            Ok(slot) => slot,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                return Some(LocalModelAdapterTerminal::ProviderLost);
            }
        };
        let terminal = if self.cancelled {
            LocalModelAdapterTerminal::Cancelled
        } else {
            slot.terminal
        };
        output.clear();
        if !self.cancelled {
            output.extend_from_slice(&slot.output);
        }
        self.next_sequence = slot.next_sequence;
        slot.input.clear();
        slot.output.clear();
        self.slot = Some(slot);
        self.cancelled = false;
        Some(terminal)
    }
    pub(super) fn retire(&mut self) -> u64 {
        self.submit.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.cancelled = true;
        let _ = self.poll(&mut Vec::new());
        self.next_sequence
    }
    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.retire();
    }
}
