//! Bounded allocation-free byte-stream framing for Conduit carrier streams.
//!
//! Provides a 2-byte big-endian length-prefixed framing codec for carriers
//! (such as USB CDC ACM serial streams) that present raw byte streams rather
//! than pre-framed messages.

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFrameError {
    /// Frame length of 0 bytes is invalid.
    ZeroLengthFrame,
    /// Frame length exceeds runtime or compile-time limits.
    FrameExceedsLimit { length: usize, limit: usize },
    /// Buffer overflow / insufficient capacity.
    BufferOverflow,
    /// Stream ended with partial frame header or body.
    TruncatedStream,
}

impl fmt::Display for StreamFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLengthFrame => write!(f, "stream frame length must be non-zero"),
            Self::FrameExceedsLimit { length, limit } => {
                write!(
                    f,
                    "stream frame length {length} exceeds maximum limit {limit}"
                )
            }
            Self::BufferOverflow => write!(f, "stream decoder buffer overflow"),
            Self::TruncatedStream => write!(f, "stream ended with truncated frame"),
        }
    }
}

/// Allocation-free streaming decoder for 2-byte big-endian length-prefixed frames.
#[derive(Debug)]
pub struct StreamFrameDecoder<const MAX: usize> {
    buf: [u8; MAX],
    read_pos: usize,
    write_pos: usize,
    runtime_max: usize,
}

impl<const MAX: usize> StreamFrameDecoder<MAX> {
    /// Creates a new streaming decoder with an explicit runtime maximum frame length.
    ///
    /// `runtime_max` must be `<= MAX` and `<= u16::MAX as usize`.
    pub fn new(runtime_max: usize) -> Result<Self, StreamFrameError> {
        let max_allowed = MAX.min(u16::MAX as usize);
        if runtime_max == 0 || runtime_max > max_allowed {
            return Err(StreamFrameError::FrameExceedsLimit {
                length: runtime_max,
                limit: max_allowed,
            });
        }
        Ok(Self {
            buf: [0u8; MAX],
            read_pos: 0,
            write_pos: 0,
            runtime_max,
        })
    }

    /// Feeds incoming stream bytes into the decoder buffer.
    pub fn accept_bytes(&mut self, input: &[u8]) -> Result<(), StreamFrameError> {
        if input.is_empty() {
            return Ok(());
        }
        self.compact_if_needed();
        if self.write_pos + input.len() > MAX {
            return Err(StreamFrameError::BufferOverflow);
        }
        self.buf[self.write_pos..self.write_pos + input.len()].copy_from_slice(input);
        self.write_pos += input.len();
        Ok(())
    }

    /// Attempts to decode the next complete frame from buffered stream data.
    ///
    /// Returns `Ok(Some(&[u8]))` when a complete frame is decoded, or `Ok(None)` if more bytes are required.
    pub fn next_frame(&mut self) -> Result<Option<&[u8]>, StreamFrameError> {
        let available = self.write_pos - self.read_pos;
        if available < 2 {
            return Ok(None);
        }

        let frame_len =
            u16::from_be_bytes([self.buf[self.read_pos], self.buf[self.read_pos + 1]]) as usize;

        if frame_len == 0 {
            return Err(StreamFrameError::ZeroLengthFrame);
        }
        if frame_len > self.runtime_max {
            return Err(StreamFrameError::FrameExceedsLimit {
                length: frame_len,
                limit: self.runtime_max,
            });
        }

        let total_needed = 2 + frame_len;
        if available < total_needed {
            return Ok(None);
        }

        let frame_start = self.read_pos + 2;
        let frame_end = frame_start + frame_len;
        self.read_pos = frame_end;

        Ok(Some(&self.buf[frame_start..frame_end]))
    }

    /// Signals EOF / stream closure and verifies that no partial frame remains.
    pub fn finish(&self) -> Result<(), StreamFrameError> {
        if self.read_pos < self.write_pos {
            Err(StreamFrameError::TruncatedStream)
        } else {
            Ok(())
        }
    }

    fn compact_if_needed(&mut self) {
        if self.read_pos > 0 {
            if self.read_pos == self.write_pos {
                self.read_pos = 0;
                self.write_pos = 0;
            } else if self.read_pos >= MAX / 2 {
                self.buf.copy_within(self.read_pos..self.write_pos, 0);
                self.write_pos -= self.read_pos;
                self.read_pos = 0;
            }
        }
    }
}

/// Encodes a frame into `output` with a 2-byte big-endian length prefix.
pub fn encode_stream_frame(
    frame: &[u8],
    runtime_max: usize,
    output: &mut [u8],
) -> Result<usize, StreamFrameError> {
    if frame.is_empty() {
        return Err(StreamFrameError::ZeroLengthFrame);
    }
    if frame.len() > runtime_max || frame.len() > u16::MAX as usize {
        return Err(StreamFrameError::FrameExceedsLimit {
            length: frame.len(),
            limit: runtime_max.min(u16::MAX as usize),
        });
    }
    let total_len = 2 + frame.len();
    if output.len() < total_len {
        return Err(StreamFrameError::BufferOverflow);
    }
    let frame_len_u16 = frame.len() as u16;
    output[0..2].copy_from_slice(&frame_len_u16.to_be_bytes());
    output[2..total_len].copy_from_slice(frame);
    Ok(total_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_single_frame() {
        let frame_data = b"hello stream";
        let mut out = [0u8; 64];
        let len = encode_stream_frame(frame_data, 100, &mut out).unwrap();
        assert_eq!(len, 2 + frame_data.len());

        let mut decoder = StreamFrameDecoder::<128>::new(100).unwrap();
        decoder.accept_bytes(&out[..len]).unwrap();
        let decoded = decoder.next_frame().unwrap().unwrap();
        assert_eq!(decoded, frame_data);
        assert!(decoder.next_frame().unwrap().is_none());
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn byte_by_byte_and_split_header_decoding() {
        let frame1 = b"frame one";
        let frame2 = b"frame two long string";

        let mut buf1 = [0u8; 64];
        let mut buf2 = [0u8; 64];
        let len1 = encode_stream_frame(frame1, 100, &mut buf1).unwrap();
        let len2 = encode_stream_frame(frame2, 100, &mut buf2).unwrap();

        let mut decoder = StreamFrameDecoder::<256>::new(100).unwrap();

        // Feed byte by byte
        for b in &buf1[..len1] {
            decoder.accept_bytes(&[*b]).unwrap();
        }
        let decoded1 = decoder.next_frame().unwrap().unwrap();
        assert_eq!(decoded1, frame1);

        // Feed frame2 with split header
        decoder.accept_bytes(&buf2[..1]).unwrap(); // first header byte
        assert!(decoder.next_frame().unwrap().is_none());
        decoder.accept_bytes(&buf2[1..5]).unwrap(); // second header byte + partial body
        assert!(decoder.next_frame().unwrap().is_none());
        decoder.accept_bytes(&buf2[5..len2]).unwrap(); // rest of body
        let decoded2 = decoder.next_frame().unwrap().unwrap();
        assert_eq!(decoded2, frame2);
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn multiple_frames_in_single_input() {
        let f1 = b"aaa";
        let f2 = b"bbbb";
        let mut buf = [0u8; 64];
        let l1 = encode_stream_frame(f1, 50, &mut buf).unwrap();
        let l2 = encode_stream_frame(f2, 50, &mut buf[l1..]).unwrap();

        let mut decoder = StreamFrameDecoder::<128>::new(50).unwrap();
        decoder.accept_bytes(&buf[..l1 + l2]).unwrap();

        assert_eq!(decoder.next_frame().unwrap().unwrap(), f1);
        assert_eq!(decoder.next_frame().unwrap().unwrap(), f2);
        assert!(decoder.next_frame().unwrap().is_none());
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn zero_length_frame_rejected() {
        let mut decoder = StreamFrameDecoder::<64>::new(32).unwrap();
        let zero_header = [0u8, 0u8];
        decoder.accept_bytes(&zero_header).unwrap();
        assert_eq!(
            decoder.next_frame().unwrap_err(),
            StreamFrameError::ZeroLengthFrame
        );
    }

    #[test]
    fn runtime_oversize_frame_rejected() {
        let mut decoder = StreamFrameDecoder::<128>::new(10).unwrap();
        let mut out = [0u8; 64];
        let len = encode_stream_frame(b"exceeds 10 bytes payload", 100, &mut out).unwrap();
        decoder.accept_bytes(&out[..len]).unwrap();
        assert!(matches!(
            decoder.next_frame().unwrap_err(),
            StreamFrameError::FrameExceedsLimit {
                length: _,
                limit: 10
            }
        ));
    }

    #[test]
    fn truncated_stream_reported_on_finish() {
        let mut decoder = StreamFrameDecoder::<64>::new(32).unwrap();
        decoder.accept_bytes(&[0, 5, b'a', b'b']).unwrap(); // expects 5 bytes body, only got 2
        assert!(decoder.next_frame().unwrap().is_none());
        assert_eq!(
            decoder.finish().unwrap_err(),
            StreamFrameError::TruncatedStream
        );
    }
}
