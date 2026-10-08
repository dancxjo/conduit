//! Structural requested-allocation accounting before canonical program decoding.
use super::*;
use core::mem::size_of;
impl PortableExpressionProgram {
    /// Includes all decoded AST owners and conservative simultaneous canonical
    /// re-encoding buffers. This scan allocates nothing; the original decoder
    /// still checks canonical identity after the quota admits its allocations.
    pub fn canonical_decode_storage_bound(
        encoded: &[u8],
    ) -> Result<usize, PortableExpressionProgramRefusal> {
        if encoded.len() > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            return Err(PortableExpressionProgramRefusal::TooLarge);
        }
        let mut scan = Scan {
            cursor: Cursor { remaining: encoded },
            bytes: 0,
            canonical_length: HEADER.len(),
        };
        scan.cursor.expect(HEADER)?;
        scan.value_type()?;
        scan.value_type()?;
        let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
        scan.node(0, &mut remaining)?;
        if !scan.cursor.remaining.is_empty() {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        // The unchanged canonical writer reserves its exact emitted length.
        scan.charge(scan.canonical_length)?;
        Ok(scan.bytes)
    }
    pub fn from_canonical_bytes_with_storage_limit(
        encoded: &[u8],
        maximum_requested_bytes: usize,
    ) -> Result<Self, PortableExpressionProgramRefusal> {
        if Self::canonical_decode_storage_bound(encoded)? > maximum_requested_bytes {
            return Err(PortableExpressionProgramRefusal::TooLarge);
        }
        Self::from_canonical_bytes(encoded)
    }
}
struct Scan<'a> {
    cursor: Cursor<'a>,
    bytes: usize,
    canonical_length: usize,
}
impl Scan<'_> {
    fn charge(&mut self, additional: usize) -> Result<(), PortableExpressionProgramRefusal> {
        self.bytes = self
            .bytes
            .checked_add(additional)
            .ok_or(PortableExpressionProgramRefusal::TooLarge)?;
        Ok(())
    }
    fn emit(&mut self, count: usize) -> Result<(), PortableExpressionProgramRefusal> {
        self.canonical_length = self
            .canonical_length
            .checked_add(count)
            .ok_or(PortableExpressionProgramRefusal::TooLarge)?;
        Ok(())
    }
    fn slots<T>(&mut self, count: usize) -> Result<(), PortableExpressionProgramRefusal> {
        self.charge(
            count
                .checked_mul(size_of::<T>())
                .ok_or(PortableExpressionProgramRefusal::TooLarge)?,
        )
    }
    fn text(&mut self) -> Result<&str, PortableExpressionProgramRefusal> {
        let length = self.cursor.length()?;
        if length > MAXIMUM_STRUCTURED_NAME_BYTES.max(crate::MAXIMUM_PLOT_SOURCE_BYTES) {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        self.emit(8)?;
        self.emit(length)?;
        self.charge(length)?;
        core::str::from_utf8(self.cursor.take(length)?)
            .map_err(|_| PortableExpressionProgramRefusal::MalformedEncoding)
    }
    fn value_type(&mut self) -> Result<(), PortableExpressionProgramRefusal> {
        let length = self.cursor.length()?;
        let encoded = self.cursor.take(length)?;
        let owners = StructuredInfoType::canonical_decode_storage_bound(encoded)?;
        self.charge(owners)?;
        // Counting and writing each own an exact temporary Type byte buffer.
        self.charge(
            length
                .checked_mul(2)
                .ok_or(PortableExpressionProgramRefusal::TooLarge)?,
        )?;
        self.emit(8)?;
        self.emit(length)
    }
    fn count(&mut self, remaining: usize) -> Result<usize, PortableExpressionProgramRefusal> {
        let count = self.cursor.bounded_node_count(remaining)?;
        self.emit(8)?;
        Ok(count)
    }
    fn nodes(
        &mut self,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<(), PortableExpressionProgramRefusal> {
        let count = self.count(*remaining)?;
        self.slots::<PortableExpressionNode>(
            count
                .max(4)
                .checked_mul(3)
                .ok_or(PortableExpressionProgramRefusal::TooLarge)?,
        )?;
        for _ in 0..count {
            self.node(depth + 1, remaining)?;
        }
        Ok(())
    }
    fn node(
        &mut self,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<(), PortableExpressionProgramRefusal> {
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        *remaining = remaining
            .checked_sub(1)
            .ok_or(PortableExpressionProgramRefusal::MalformedEncoding)?;
        self.value_type()?;
        self.emit(1)?;
        match self.cursor.byte()? {
            0 => Ok(()),
            1 => {
                self.text()?;
                Ok(())
            }
            2 => {
                self.slots::<PortableExpressionNode>(1)?;
                self.node(depth + 1, remaining)?;
                self.emit(1)?;
                match self.cursor.byte()? {
                    0 => {
                        self.text()?;
                        Ok(())
                    }
                    1 => {
                        self.cursor.take(2)?;
                        self.emit(2)
                    }
                    _ => Err(PortableExpressionProgramRefusal::MalformedEncoding),
                }
            }
            3 => {
                unary(self.cursor.byte()?)?;
                self.emit(1)?;
                self.slots::<PortableExpressionNode>(1)?;
                self.node(depth + 1, remaining)
            }
            4 => {
                binary(self.cursor.byte()?)?;
                self.emit(2)?;
                if !matches!(self.cursor.byte()?, 0 | 1) {
                    return Err(PortableExpressionProgramRefusal::MalformedEncoding);
                }
                self.slots::<PortableExpressionNode>(2)?;
                self.node(depth + 1, remaining)?;
                self.node(depth + 1, remaining)
            }
            5 => {
                self.slots::<PortableExpressionNode>(3)?;
                for _ in 0..3 {
                    self.node(depth + 1, remaining)?;
                }
                Ok(())
            }
            6 | 8 => self.nodes(depth, remaining),
            7 => {
                let count = self.count(*remaining)?;
                self.slots::<(String, PortableExpressionNode)>(count)?;
                for _ in 0..count {
                    let name = self.text()?;
                    if name.is_empty() || name.len() > MAXIMUM_STRUCTURED_NAME_BYTES {
                        return Err(PortableExpressionProgramRefusal::MalformedEncoding);
                    }
                    self.node(depth + 1, remaining)?;
                }
                Ok(())
            }
            9 => {
                self.text()?;
                self.slots::<PortableExpressionNode>(1)?;
                self.node(depth + 1, remaining)
            }
            10 => {
                self.text()?;
                self.nodes(depth, remaining)
            }
            _ => Err(PortableExpressionProgramRefusal::MalformedEncoding),
        }
    }
}
