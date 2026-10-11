//! Exact duplex calls and one-event completion routing.
use super::*;

impl NativeUsbDuplexHost {
    pub(crate) fn subordinate_nodes(&self) -> [NodeId; 3] {
        [self.control_node, self.read_node, self.write_node]
    }

    pub(crate) fn validate_settlement_idle(&self) -> Result<(), UsbNativeProtocolRefusal> {
        self.host.validate_settlement_idle().map_err(native_refusal)
    }

    pub fn control_invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbNativeProtocolRefusal> {
        if node != self.control_node || call != HostCallId(0) {
            return Err(UsbNativeProtocolRefusal::Control(
                UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding),
            ));
        }
        self.host
            .control_invoke(self.control_index, request, input)
            .map_err(native_refusal)
    }
    pub fn read_begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeProtocolRefusal> {
        if node != self.read_node || call != HostCallId(0) {
            return Err(read_binding());
        }
        self.host
            .begin_independent_read(self.read_index, request, input)
            .map_err(native_refusal)
    }
    pub fn read_poll(&mut self) -> Result<Option<&[u8]>, UsbNativeProtocolRefusal> {
        self.host
            .poll_endpoint(self.read_index)
            .map_err(native_refusal)
    }
    pub fn write_begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeProtocolRefusal> {
        if node != self.write_node || call != HostCallId(0) {
            return Err(write_binding());
        }
        self.host
            .begin_independent_write(self.write_index, request, input)
            .map_err(native_refusal)
    }
    pub fn write_poll(&mut self) -> Result<Option<&[u8]>, UsbNativeProtocolRefusal> {
        self.host
            .poll_write(self.write_index)
            .map_err(native_refusal)
    }
    /// Consume at most one event and identify its original selected operation.
    pub fn poll(&mut self) -> Result<Option<(NodeId, &[u8])>, UsbNativeProtocolRefusal> {
        let Some((index, bytes)) = self.host.poll_independent().map_err(native_refusal)? else {
            return Ok(None);
        };
        let node = if index == self.read_index {
            self.read_node
        } else if index == self.write_index {
            self.write_node
        } else {
            return Err(read_binding());
        };
        Ok(Some((node, bytes)))
    }
    pub fn cancel(&mut self) -> Result<(), UsbNativeProtocolRefusal> {
        self.host.cancel().map_err(native_refusal)
    }
}
