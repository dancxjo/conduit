//! Bounded loopback carrier for the existing browser admission protocol.
use conduit_std_host::{
    browser_admission::{
        decode_browser_admission_frame, encode_browser_admission_frame, BrowserAdmissionEgress,
        BrowserAdmissionIngress, MAX_BROWSER_ADMISSION_FRAME_BYTES,
    },
    websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener},
};
use std::time::Duration;

pub(super) struct Listener(NativeWebSocketListener);
pub(super) struct Socket {
    line: NativeWebSocketLine,
    input: Box<[u8]>,
    output: Box<[u8]>,
}
impl Listener {
    pub(super) fn bind() -> Result<Self, String> {
        NativeWebSocketListener::bind_loopback(MAX_BROWSER_ADMISSION_FRAME_BYTES as u32)
            .map(Self)
            .map_err(debug)
    }
    pub(super) fn url(&self) -> Result<String, String> {
        self.0.url().map_err(debug)
    }
    pub(super) fn accept(&self, timeout: Duration) -> Result<Option<Socket>, String> {
        match self.0.accept_with_timeout(timeout) {
            Ok(line) => Ok(Some(Socket {
                line,
                input: vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES].into_boxed_slice(),
                output: vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES].into_boxed_slice(),
            })),
            Err(NativeWebSocketError::AcceptDeadline) => Ok(None),
            Err(error) => Err(debug(error)),
        }
    }
}
impl Socket {
    pub(super) fn receive(
        &mut self,
        timeout: Duration,
    ) -> Result<(BrowserAdmissionIngress, u32), String> {
        self.line
            .set_read_timeout(Some(timeout.max(Duration::from_millis(1))))
            .map_err(debug)?;
        let bytes = self.line.receive_binary(&mut self.input).map_err(debug)?;
        Ok((
            decode_browser_admission_frame(&self.input[..bytes]).map_err(debug)?,
            bytes as u32,
        ))
    }
    pub(super) fn send(&mut self, frame: &BrowserAdmissionEgress) -> Result<(), String> {
        let length = encode_browser_admission_frame(frame, &mut self.output).map_err(debug)?;
        self.line.send_binary(&self.output[..length]).map_err(debug)
    }
    pub(super) fn close(&mut self) {
        let _ = self.line.close();
    }
}
fn debug(error: impl std::fmt::Debug) -> String {
    format!("browser admission carrier: {error:?}")
}
