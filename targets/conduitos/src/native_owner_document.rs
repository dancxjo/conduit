//! Finite owner-to-native JSON documents over one pinned WebSocket Line.

use alloc::vec;
use conduit_wire::BoundedDocumentAssembly;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES, WebSocketError};

#[derive(Debug)]
pub(crate) enum DocumentRefusal {
    Receive(WebSocketError),
    Bound,
    Decode,
}

pub(crate) fn receive<T: DeserializeOwned>(
    line: &mut dyn BinaryWebSocketIo,
    maximum_bytes: usize,
) -> Result<T, DocumentRefusal> {
    let mut frame = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
    let mut length = line
        .receive_binary(&mut frame)
        .map_err(DocumentRefusal::Receive)?;
    if length == 0 {
        return Err(DocumentRefusal::Bound);
    }
    if frame[0] == b'{' {
        if length > maximum_bytes {
            return Err(DocumentRefusal::Bound);
        }
        return serde_json::from_slice(&frame[..length]).map_err(|_| DocumentRefusal::Decode);
    }
    let mut assembly = BoundedDocumentAssembly::new(maximum_bytes, 16);
    for _ in 0..16 {
        let complete = assembly
            .push(&frame[..length])
            .map_err(|_| DocumentRefusal::Bound)?;
        if complete {
            let (document, expected_digest) = assembly.completed().ok_or(DocumentRefusal::Bound)?;
            if Sha256::digest(document).as_slice() != expected_digest {
                return Err(DocumentRefusal::Decode);
            }
            return serde_json::from_slice(document).map_err(|_| DocumentRefusal::Decode);
        }
        length = line
            .receive_binary(&mut frame)
            .map_err(DocumentRefusal::Receive)?;
    }
    Err(DocumentRefusal::Bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use conduit_wire::encode_bounded_document_chunk;

    struct Frames {
        frames: Vec<Vec<u8>>,
        next: usize,
    }

    impl BinaryWebSocketIo for Frames {
        fn send_binary(&mut self, _: &[u8]) -> Result<(), WebSocketError> {
            Err(WebSocketError::FrameWrite)
        }

        fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError> {
            if output.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
                return Err(WebSocketError::ResponseTooLarge);
            }
            let frame = self
                .frames
                .get(self.next)
                .ok_or(WebSocketError::FrameRead)?;
            if frame.len() > output.len() {
                return Err(WebSocketError::ResponseTooLarge);
            }
            output[..frame.len()].copy_from_slice(frame);
            self.next += 1;
            Ok(frame.len())
        }
    }

    #[test]
    fn large_owner_document_crosses_finite_native_frames() {
        let value = serde_json::json!({"face":"x".repeat(20_000)});
        let document = serde_json::to_vec(&value).unwrap();
        let digest: [u8; 32] = Sha256::digest(&document).into();
        let mut frame = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
        let mut frames = Vec::new();
        let mut offset = 0;
        while offset < document.len() {
            let (length, next) =
                encode_bounded_document_chunk(&mut frame, &document, offset, &digest).unwrap();
            frames.push(frame[..length].to_vec());
            offset = next;
        }
        let mut line = Frames { frames, next: 0 };
        let received: serde_json::Value = receive(&mut line, 64 * 1024).unwrap();
        assert_eq!(received, value);
        assert_eq!(line.next, line.frames.len());
    }

    #[test]
    fn truncated_or_tampered_owner_document_refuses() {
        let value = serde_json::json!({"face":"x".repeat(20_000)});
        let document = serde_json::to_vec(&value).unwrap();
        let digest: [u8; 32] = Sha256::digest(&document).into();
        let mut frame = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
        let (length, _) = encode_bounded_document_chunk(&mut frame, &document, 0, &digest).unwrap();
        let mut line = Frames {
            frames: vec![frame[..length].to_vec()],
            next: 0,
        };
        assert!(matches!(
            receive::<serde_json::Value>(&mut line, 64 * 1024),
            Err(DocumentRefusal::Receive(WebSocketError::FrameRead))
        ));
        let mut frames = Vec::new();
        let mut offset = 0;
        while offset < document.len() {
            let (length, next) =
                encode_bounded_document_chunk(&mut frame, &document, offset, &digest).unwrap();
            frames.push(frame[..length].to_vec());
            offset = next;
        }
        frames[1][44] ^= 1;
        let mut line = Frames { frames, next: 0 };
        assert!(matches!(
            receive::<serde_json::Value>(&mut line, 64 * 1024),
            Err(DocumentRefusal::Decode)
        ));
    }
}
