//! Side-effect-free output admission observations for one transactional Step.
use super::{PortId, StepIo};

impl<const PORTS: usize> StepIo<PORTS> {
    pub fn output_ready(&self, port: PortId) -> bool {
        self.output_maximum_bytes
            .get(usize::from(port.0))
            .copied()
            .flatten()
            .is_some()
    }

    /// A complete value must fit before consuming its input or Host completion.
    /// Observing partial capacity does not stage an output or poison the Step.
    pub fn output_ready_for(&self, port: PortId, byte_len: u32) -> bool {
        let index = usize::from(port.0);
        self.output_maximum_bytes
            .get(index)
            .copied()
            .flatten()
            .is_some_and(|maximum| byte_len <= maximum)
            && self.outputs.get(index).is_some_and(Option::is_none)
    }
}
