//! Finite owner-to-native JSON response on the authenticated WebSocket Line.

use conduit_wire::encode_bounded_document_chunk;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::native_lines::MAX_NATIVE_FRAME_BYTES;

pub(super) const MAX_OWNER_DOCUMENT_BYTES: usize = 64 * 1024;
pub(super) const MAX_HOSTED_OWNER_DOCUMENT_BYTES: usize = 128 * 1024;

pub(super) fn send<T: Serialize>(
    line: &mut impl super::BinaryRoute,
    value: &T,
) -> Result<(), String> {
    send_bounded(line, value, MAX_OWNER_DOCUMENT_BYTES)
}

pub(super) fn send_hosted<T: Serialize>(
    line: &mut impl super::BinaryRoute,
    value: &T,
) -> Result<(), String> {
    send_bounded(line, value, MAX_HOSTED_OWNER_DOCUMENT_BYTES)
}

fn send_bounded<T: Serialize>(
    line: &mut impl super::BinaryRoute,
    value: &T,
    maximum: usize,
) -> Result<(), String> {
    let document = serde_json::to_vec(value)
        .map_err(|error| format!("encode native owner document: {error}"))?;
    if document.is_empty() || document.len() > maximum {
        return Err("native owner document exceeded finite bound".into());
    }
    if document.len() <= MAX_NATIVE_FRAME_BYTES {
        return line.send_binary(&document);
    }
    let digest: [u8; 32] = Sha256::digest(&document).into();
    let mut frame = vec![0; MAX_NATIVE_FRAME_BYTES];
    let mut offset = 0;
    while offset < document.len() {
        let (length, next) = encode_bounded_document_chunk(&mut frame, &document, offset, &digest)
            .map_err(|_| "native owner document chunk refused")?;
        line.send_binary(&frame[..length])?;
        offset = next;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_wire::BoundedDocumentAssembly;

    #[derive(Default)]
    struct Frames(Vec<Vec<u8>>);

    impl super::super::BinaryRoute for Frames {
        fn send_binary(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0.push(bytes.to_vec());
            Ok(())
        }

        fn receive_binary(&mut self, _: &mut [u8]) -> Result<usize, String> {
            Err("not used".into())
        }
    }

    #[test]
    fn large_owner_face_remains_complete_across_native_frames() {
        let value = serde_json::json!({"face":"x".repeat(20_000)});
        let mut line = Frames::default();
        send(&mut line, &value).unwrap();
        assert!(line.0.len() > 1);
        let mut assembly = BoundedDocumentAssembly::new(MAX_OWNER_DOCUMENT_BYTES, 16);
        for (index, frame) in line.0.iter().enumerate() {
            assert!(frame.len() <= MAX_NATIVE_FRAME_BYTES);
            assert_eq!(assembly.push(frame).unwrap(), index + 1 == line.0.len());
        }
        let (document, digest) = assembly.completed().unwrap();
        assert_eq!(Sha256::digest(document).as_slice(), digest);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(document).unwrap(),
            value
        );
    }
    #[test]
    fn hosted_control_document_is_explicitly_bounded_without_growing_native_profile() {
        // Transport extent fixture only: no Face, route, Host or delivery claim.
        let value = serde_json::json!({"payload":"x".repeat(90_000)});
        let mut native = Frames::default();
        assert!(send(&mut native, &value).is_err());
        assert!(native.0.is_empty());
        let mut hosted = Frames::default();
        send_hosted(&mut hosted, &value).unwrap();
        let mut assembly = BoundedDocumentAssembly::new(MAX_HOSTED_OWNER_DOCUMENT_BYTES, 32);
        for (index, frame) in hosted.0.iter().enumerate() {
            assert!(frame.len() <= MAX_NATIVE_FRAME_BYTES);
            assert_eq!(assembly.push(frame).unwrap(), index + 1 == hosted.0.len());
        }
        let (document, digest) = assembly.completed().unwrap();
        assert_eq!(Sha256::digest(document).as_slice(), digest);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(document).unwrap(),
            value
        );
        let mut oversized = Frames::default();
        assert!(send_hosted(
            &mut oversized,
            &serde_json::json!({"payload":"x".repeat(MAX_HOSTED_OWNER_DOCUMENT_BYTES)})
        )
        .is_err());
        assert!(oversized.0.is_empty());
    }
}
