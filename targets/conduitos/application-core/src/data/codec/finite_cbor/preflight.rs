//! Allocation-free validation of the complete finite CBOR carrier before Serde.
use super::Refusal;
#[derive(Clone, Copy)]
enum Scope {
    Root,
    Show,
    Mask,
    Plan,
    Other,
}
pub(super) fn validate(bytes: &[u8], maximum_plan: usize) -> Result<(), Refusal> {
    if bytes.is_empty() || bytes.len() > super::super::MAX_DECODED_JSON_BYTES {
        return Err(Refusal::Bounds);
    }
    let mut scan = Scan {
        bytes,
        at: 0,
        maximum_plan,
    };
    scan.value(0, Scope::Root)?;
    if scan.at != bytes.len() {
        return Err(Refusal::Malformed);
    }
    Ok(())
}
struct Scan<'a> {
    bytes: &'a [u8],
    at: usize,
    maximum_plan: usize,
}
impl<'a> Scan<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Refusal> {
        let end = self.at.checked_add(n).ok_or(Refusal::Bounds)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Refusal::Malformed)?;
        self.at = end;
        Ok(bytes)
    }
    fn header(&mut self) -> Result<(u8, u64), Refusal> {
        let byte = self.take(1)?[0];
        let low = byte & 31;
        let argument = match low {
            0..=23 => u64::from(low),
            24 => u64::from(self.take(1)?[0]),
            25 => u64::from(u16::from_be_bytes(self.take(2)?.try_into().unwrap())),
            26 => u64::from(u32::from_be_bytes(self.take(4)?.try_into().unwrap())),
            27 => u64::from_be_bytes(self.take(8)?.try_into().unwrap()),
            _ => return Err(Refusal::Malformed), // No indefinite lengths or breaks.
        };
        Ok((byte >> 5, argument))
    }
    fn text(&mut self) -> Result<&'a str, Refusal> {
        let (major, n) = self.header()?;
        if major != 3 {
            return Err(Refusal::Malformed);
        }
        let n = usize::try_from(n).map_err(|_| Refusal::Bounds)?;
        core::str::from_utf8(self.take(n)?).map_err(|_| Refusal::Malformed)
    }
    fn value(&mut self, depth: usize, scope: Scope) -> Result<(), Refusal> {
        if depth > 64 {
            return Err(Refusal::Bounds);
        }
        let start = self.at;
        let initial = *self.bytes.get(self.at).ok_or(Refusal::Malformed)?;
        let (major, argument) = self.header()?;
        if matches!(scope, Scope::Root | Scope::Show | Scope::Mask) && major != 5 {
            return Err(Refusal::Malformed);
        }
        match major {
            0 | 1 => {}
            2 | 3 => {
                let n = usize::try_from(argument).map_err(|_| Refusal::Bounds)?;
                let bytes = self.take(n)?;
                if major == 3 {
                    core::str::from_utf8(bytes).map_err(|_| Refusal::Malformed)?;
                }
            }
            4 | 5 => {
                if depth >= 64 {
                    return Err(Refusal::Bounds);
                }
                let count = usize::try_from(argument).map_err(|_| Refusal::Bounds)?;
                let width = if major == 5 { 2 } else { 1 };
                if count > (self.bytes.len() - self.at) / width {
                    return Err(Refusal::Malformed);
                }
                for _ in 0..count {
                    let child = if major == 5 {
                        if matches!(scope, Scope::Root | Scope::Show | Scope::Mask) {
                            match (scope, self.text()?) {
                                (Scope::Root, "show") => Scope::Show,
                                (Scope::Show, "planned_mask") => Scope::Mask,
                                (Scope::Mask, "plan") => Scope::Plan,
                                _ => Scope::Other,
                            }
                        } else {
                            self.value(depth + 1, Scope::Other)?;
                            Scope::Other
                        }
                    } else {
                        Scope::Other
                    };
                    self.value(depth + 1, child)?;
                }
            }
            6 => {
                // The closed typed carrier uses only the standard u128/i128
                // bignum tags. Bound their complete byte payload before decode.
                if !matches!(argument, 2 | 3) {
                    return Err(Refusal::Malformed);
                }
                let (major, length) = self.header()?;
                if major != 2 || length > 16 {
                    return Err(Refusal::Malformed);
                }
                self.take(length as usize)?;
            }
            7 => match initial & 31 {
                20..=22 => {}
                25 if argument & 0x7c00 != 0x7c00 => {}
                26 if argument & 0x7f80_0000 != 0x7f80_0000 => {}
                27 if argument & 0x7ff0_0000_0000_0000 != 0x7ff0_0000_0000_0000 => {}
                _ => return Err(Refusal::Malformed),
            },
            _ => return Err(Refusal::Malformed),
        }
        if matches!(scope, Scope::Plan) && self.at - start > self.maximum_plan {
            return Err(Refusal::Bounds);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
