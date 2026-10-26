//! Diagnostic-only syntax counts over retained private bytes, never admission.
//! No values, names, paths or payload bytes are retained or printed.
const LIMIT: usize = 128 * 1024;
const PREFIX: &[u8] = b"CDCI\x01";
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Shape {
    pub bytes: usize,
    pub major: [u64; 8],
    pub text_bytes: u64,
    pub key_text_bytes: u64,
    pub byte_string_bytes: u64,
    pub empty_arrays: u64,
    pub u8_arrays: [u64; 8],
    pub u8_elements: u64,
    pub u8_encoded_bytes: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Bounds,
    Encoding,
    Depth,
}
struct Scan<'a> {
    input: &'a [u8],
    position: usize,
    shape: Shape,
}
pub(super) fn inspect(bytes: &[u8]) -> Result<Shape, Error> {
    if bytes.len() > LIMIT {
        return Err(Error::Bounds);
    }
    let input = bytes.strip_prefix(PREFIX).ok_or(Error::Encoding)?;
    let mut scan = Scan {
        input,
        position: 0,
        shape: Shape {
            bytes: bytes.len(),
            ..Shape::default()
        },
    };
    scan.item(64, false)?;
    if scan.position != input.len() {
        return Err(Error::Encoding);
    }
    Ok(scan.shape)
}
impl Scan<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], Error> {
        let end = self.position.checked_add(count).ok_or(Error::Bounds)?;
        let value = self.input.get(self.position..end).ok_or(Error::Encoding)?;
        self.position = end;
        Ok(value)
    }
    fn header(&mut self) -> Result<(u8, Option<u64>), Error> {
        let byte = self.take(1)?[0];
        let major = byte >> 5;
        self.shape.major[usize::from(major)] += 1;
        let argument = match byte & 31 {
            n @ 0..=23 => Some(u64::from(n)),
            n @ 24..=27 => {
                let count = 1usize << (n - 24);
                let mut value = 0;
                for byte in self.take(count)? {
                    value = (value << 8) | u64::from(*byte);
                }
                Some(value)
            }
            31 => None,
            _ => return Err(Error::Encoding),
        };
        Ok((major, argument))
    }
    fn stop(&mut self) -> Result<bool, Error> {
        if self.input.get(self.position) == Some(&255) {
            self.header()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    // Returns true only for one direct untagged unsigned integer <=255.
    // Homogeneous arrays are candidates, not a claim about Rust Vec versus tuple.
    fn item(&mut self, depth: u8, key: bool) -> Result<bool, Error> {
        let start = self.position;
        let mut tagged = false;
        let (major, argument) = loop {
            let head = self.header()?;
            if head.0 != 6 {
                break head;
            }
            if head.1.is_none() {
                return Err(Error::Encoding);
            }
            tagged = true;
        };
        match major {
            0 | 1 => {
                if argument.is_none() {
                    return Err(Error::Encoding);
                }
            }
            2 | 3 => self.string(major, argument, key)?,
            4 | 5 => {
                if depth == 0 {
                    return Err(Error::Depth);
                }
                if argument.is_some_and(|n| n > (self.input.len() - self.position) as u64) {
                    return Err(Error::Encoding);
                }
                let mut count = 0u64;
                let mut bytes_only = true;
                loop {
                    if let Some(total) = argument {
                        if count == total {
                            break;
                        }
                    } else if self.stop()? {
                        break;
                    }
                    if major == 5 {
                        self.item(depth - 1, true)?;
                        self.item(depth - 1, false)?;
                    } else {
                        bytes_only &= self.item(depth - 1, false)?;
                    }
                    count += 1;
                }
                if major == 4 {
                    if count == 0 {
                        self.shape.empty_arrays += 1;
                    } else if bytes_only {
                        let bin = match count {
                            1..=15 => 0,
                            16..=31 => 1,
                            32 => 2,
                            33..=63 => 3,
                            64..=127 => 4,
                            128..=255 => 5,
                            256..=1023 => 6,
                            _ => 7,
                        };
                        self.shape.u8_arrays[bin] += 1;
                        self.shape.u8_elements += count;
                        self.shape.u8_encoded_bytes += (self.position - start) as u64;
                    }
                }
            }
            7 => {
                if argument.is_none() {
                    return Err(Error::Encoding);
                }
            }
            _ => return Err(Error::Encoding),
        }
        Ok(!tagged && major == 0 && argument.is_some_and(|n| n <= 255))
    }
    fn string(&mut self, major: u8, length: Option<u64>, key: bool) -> Result<(), Error> {
        if let Some(length) = length {
            let length = usize::try_from(length).map_err(|_| Error::Bounds)?;
            self.take(length)?;
            if major == 2 {
                self.shape.byte_string_bytes += length as u64;
            } else {
                self.shape.text_bytes += length as u64;
                if key {
                    self.shape.key_text_bytes += length as u64;
                }
            }
        } else {
            while !self.stop()? {
                let (part_major, part_length) = self.header()?;
                if part_major != major || part_length.is_none() {
                    return Err(Error::Encoding);
                }
                self.string(major, part_length, key)?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::vec;
    fn shape(bytes: &[u8]) -> Result<Shape, Error> {
        let mut input = PREFIX.to_vec();
        input.extend_from_slice(bytes);
        inspect(&input)
    }
    #[test]
    fn map_counts_lengths_without_retaining_keys_or_values() {
        // {"a":[0,24,255], "b":bytes(3), "c":"hello"}
        let s = shape(&[
            0xa3, 0x61, b'a', 0x83, 0, 0x18, 24, 0x18, 255, 0x61, b'b', 0x43, 1, 2, 3, 0x61, b'c',
            0x65, b'h', b'e', b'l', b'l', b'o',
        ])
        .unwrap();
        assert_eq!(s.major, [3, 0, 1, 4, 1, 1, 0, 0]);
        assert_eq!(s.text_bytes, 8);
        assert_eq!(s.key_text_bytes, 3);
        assert_eq!(s.byte_string_bytes, 3);
        assert_eq!(s.u8_arrays, [1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(s.u8_elements, 3);
        assert_eq!(s.u8_encoded_bytes, 6);
    }
    #[test]
    fn indefinite_and_tagged_tokens_stay_finite_and_distinct() {
        let s = shape(&[
            0x9f, 0x82, 1, 2, 0x80, 0xc0, 3, 0x7f, 0x61, b'a', 0x61, b'b', 0xff, 0xff,
        ])
        .unwrap();
        assert_eq!(s.u8_arrays, [1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(s.empty_arrays, 1);
        assert_eq!(s.u8_elements, 2);
        assert_eq!(s.text_bytes, 2);
        let s = shape(&[0xbf, 0x61, b'k', 0x9f, 1, 2, 0xff, 0xff]).unwrap();
        assert_eq!(s.key_text_bytes, 1);
        assert_eq!(s.u8_elements, 2);
    }
    #[test]
    fn bounds_depth_and_malformed_never_escape_the_diagnostic() {
        assert_eq!(inspect(&[]), Err(Error::Encoding));
        assert_eq!(inspect(&vec![0; LIMIT + 1]), Err(Error::Bounds));
        for malformed in [
            &[0x82, 1][..],
            &[0xff][..],
            &[0x18][..],
            &[0x61][..],
            &[0x1c][..],
            &[0xbf, 1, 0xff][..],
            &[1, 2][..],
        ] {
            assert!(shape(malformed).is_err());
        }
        let mut legal = vec![0x81; 64];
        legal.push(1);
        assert!(shape(&legal).is_ok());
        legal.insert(0, 0x81);
        assert_eq!(shape(&legal), Err(Error::Depth));
        let mut tags = vec![0xc0; 4096];
        tags.push(1);
        assert!(shape(&tags).is_ok());
    }
    #[test]
    fn long_array_histogram_and_empty_arrays_do_not_invent_byte_types() {
        for (count, bin) in [
            (16, 1),
            (32, 2),
            (33, 3),
            (64, 4),
            (128, 5),
            (256, 6),
            (1024, 7),
        ] {
            let mut input = if count < 256 {
                vec![0x98, count as u8]
            } else {
                vec![0x99, (count >> 8) as u8, count as u8]
            };
            input.resize(input.len() + count, 1);
            let s = shape(&input).unwrap();
            assert_eq!(s.u8_arrays[bin], 1);
            assert_eq!(s.u8_elements, count as u64);
        }
        assert_eq!(shape(&[0x80]).unwrap().u8_arrays, [0; 8]);
        assert_eq!(shape(&[0x82, 0x20, 1]).unwrap().u8_arrays, [0; 8]);
    }
}
