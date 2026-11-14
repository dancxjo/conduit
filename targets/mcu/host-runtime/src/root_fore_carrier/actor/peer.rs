//! A real independent Wire peer on the actual framed MessageTransport boundary.
use super::*;
use std::{collections::VecDeque, vec::Vec};
pub struct Peer {
    pub machines: [Machine; 2],
    pub queue: VecDeque<Vec<u8>>,
    pub writes: usize,
    pub delivered: Vec<Vec<u8>>,
    pub fail: bool,
}
impl Peer {
    pub fn new(bindings: [Binding; 2]) -> Self {
        Self {
            machines: [
                Machine::new(bindings[0].clone(), SessionRole::Source).unwrap(),
                Machine::new(bindings[1].clone(), SessionRole::Sink).unwrap(),
            ],
            queue: VecDeque::new(),
            writes: 0,
            delivered: Vec::new(),
            fail: false,
        }
    }
    pub fn emit(&mut self, index: usize, message: Message<'_>) {
        let frame = self.machines[index].binding().frame(message);
        let mut bytes = std::vec![0;128];
        let n = conduit_wire::encode_external_fore_session_frame_into(frame, &mut bytes, 8, 128)
            .unwrap();
        self.machines[index].admit_outbound(frame).unwrap();
        bytes.truncate(n);
        self.queue.push_back(bytes);
    }
}
