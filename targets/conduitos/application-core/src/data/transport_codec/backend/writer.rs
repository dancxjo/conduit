//! Checked slice cursor and allocation-free JSON scalar spelling.
use core::fmt::{self, Write};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Error {
    Capacity,
    Malformed,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Capacity => "private JSON capacity",
            Self::Malformed => "private JSON serialization",
        })
    }
}
impl serde::ser::StdError for Error {}
impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self::Malformed
    }
}
pub(super) struct Writer<'a> {
    pub(super) output: &'a mut [u8],
    pub(super) at: usize,
    pub(super) key: bool,
}
impl Writer<'_> {
    pub(super) fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self.at.checked_add(bytes.len()).ok_or(Error::Capacity)?;
        self.output
            .get_mut(self.at..end)
            .ok_or(Error::Capacity)?
            .copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }
    pub(super) fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.bytes(&[byte])
    }
    pub(super) fn number(&mut self, value: impl fmt::Display) -> Result<(), Error> {
        if self.key {
            return self.quoted(value);
        }
        write!(self, "{value}").map_err(|_| Error::Capacity)
    }
    pub(super) fn scientific(&mut self, value: impl fmt::LowerExp) -> Result<(), Error> {
        struct Scientific<T>(T);
        impl<T: fmt::LowerExp> fmt::Display for Scientific<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:e}", self.0)
            }
        }
        self.number(Scientific(value))
    }
    pub(super) fn quoted(&mut self, value: impl fmt::Display) -> Result<(), Error> {
        self.byte(b'"')?;
        write!(Escaped(self), "{value}").map_err(|_| Error::Capacity)?;
        self.byte(b'"')
    }
    pub(super) fn string(&mut self, value: &str) -> Result<(), Error> {
        self.byte(b'"')?;
        self.content(value)?;
        self.byte(b'"')
    }
    fn content(&mut self, value: &str) -> Result<(), Error> {
        // UTF-8 bytes >= 0x20 are emitted unchanged. Escapes concern ASCII only.
        let mut start = 0;
        for (at, byte) in value.bytes().enumerate() {
            let escape: &[u8] = match byte {
                b'"' => b"\\\"",
                b'\\' => b"\\\\",
                b'\n' => b"\\n",
                b'\r' => b"\\r",
                b'\t' => b"\\t",
                8 => b"\\b",
                12 => b"\\f",
                0..=31 => {
                    self.bytes(&value.as_bytes()[start..at])?;
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    self.bytes(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[(byte >> 4) as usize],
                        HEX[(byte & 15) as usize],
                    ])?;
                    start = at + 1;
                    continue;
                }
                _ => continue,
            };
            self.bytes(&value.as_bytes()[start..at])?;
            self.bytes(escape)?;
            start = at + 1;
        }
        self.bytes(&value.as_bytes()[start..])
    }
    pub(super) fn compound(&mut self, open: &[u8]) -> Result<(), Error> {
        if self.key {
            return Err(Error::Malformed);
        }
        self.bytes(open)
    }
}
impl fmt::Write for Writer<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes(value.as_bytes()).map_err(|_| fmt::Error)
    }
}
struct Escaped<'a, 'b>(&'a mut Writer<'b>);
impl fmt::Write for Escaped<'_, '_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0.content(value).map_err(|_| fmt::Error)
    }
}
