//! Bounded preparation-time decoding of the existing semantic contract bytes.
use super::*;
type Error = ConstraintDefinitionError;
const MAX_BYTES: usize = 1024 * 1024;
impl CheckedValueContract {
    /// Reconstruct an admitted contract without a second semantic encoding.
    /// This preparation entrance allocates; value validation remains borrowed.
    pub fn from_identity_bytes(encoded: &[u8]) -> Result<Self, Error> {
        if encoded.len() > MAX_BYTES {
            return Err(Error::InvalidCanonicalEncoding);
        }
        let mut cursor = Cursor(encoded);
        if cursor.take(b"conduit.value-contract@3\0".len())? != b"conduit.value-contract@3\0" {
            return Err(Error::InvalidCanonicalEncoding);
        }
        let kind = cursor.kind()?;
        let maximum = cursor.u32()?;
        let count = cursor.count(MAX_VALUE_CONSTRAINTS)?;
        let mut constraints = Vec::with_capacity(count);
        for _ in 0..count {
            constraints.push(cursor.constraint()?);
        }
        if !cursor.0.is_empty() {
            return Err(Error::InvalidCanonicalEncoding);
        }
        let value = Self::new(kind, maximum, constraints)?;
        if value.identity_bytes() != encoded {
            return Err(Error::InvalidCanonicalEncoding);
        }
        Ok(value)
    }
}
struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let (value, tail) = self
            .0
            .split_at_checked(n)
            .ok_or(Error::InvalidCanonicalEncoding)?;
        self.0 = tail;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| Error::InvalidCanonicalEncoding)?,
        ))
    }
    fn count(&mut self, maximum: usize) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        if n > maximum {
            return Err(Error::InvalidCanonicalEncoding);
        }
        Ok(n)
    }
    fn bytes(&mut self) -> Result<Vec<u8>, Error> {
        let n = self.count(MAX_BYTES)?;
        Ok(self.take(n)?.to_vec())
    }
    fn kind(&mut self) -> Result<KindId, Error> {
        let bytes = self.bytes()?;
        let text = core::str::from_utf8(&bytes).map_err(|_| Error::InvalidCanonicalEncoding)?;
        Ok(KindId::from(text))
    }
    fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidCanonicalEncoding),
        }
    }
    fn optional<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<Option<T>, Error> {
        if self.boolean()? {
            Ok(Some(read(self)?))
        } else {
            Ok(None)
        }
    }
    fn endpoint(&mut self) -> Result<IntervalEndpoint, Error> {
        match self.byte()? {
            0 => Ok(IntervalEndpoint::Inclusive),
            1 => Ok(IntervalEndpoint::Exclusive),
            _ => Err(Error::InvalidCanonicalEncoding),
        }
    }
    fn quantity(&mut self) -> Result<Box<crate::Quantity>, Error> {
        crate::Quantity::decode(self.take(crate::QUANTITY_ENCODED_LEN)?)
            .map(Box::new)
            .map_err(|_| Error::InvalidCanonicalEncoding)
    }
    fn constraint(&mut self) -> Result<ValueConstraint, Error> {
        use ValueConstraint as C;
        Ok(match self.byte()? {
            0 => C::ByteLength {
                minimum: self.u32()?,
                maximum: self.u32()?,
            },
            1 => C::UnsignedRange {
                minimum: self.optional(|c| {
                    Ok(u64::from_le_bytes(
                        c.take(8)?
                            .try_into()
                            .map_err(|_| Error::InvalidCanonicalEncoding)?,
                    ))
                })?,
                maximum: self.optional(|c| {
                    Ok(u64::from_le_bytes(
                        c.take(8)?
                            .try_into()
                            .map_err(|_| Error::InvalidCanonicalEncoding)?,
                    ))
                })?,
                minimum_endpoint: self.endpoint()?,
                maximum_endpoint: self.endpoint()?,
            },
            2 => C::SignedRange {
                minimum: self.optional(|c| {
                    Ok(i64::from_le_bytes(
                        c.take(8)?
                            .try_into()
                            .map_err(|_| Error::InvalidCanonicalEncoding)?,
                    ))
                })?,
                maximum: self.optional(|c| {
                    Ok(i64::from_le_bytes(
                        c.take(8)?
                            .try_into()
                            .map_err(|_| Error::InvalidCanonicalEncoding)?,
                    ))
                })?,
                minimum_endpoint: self.endpoint()?,
                maximum_endpoint: self.endpoint()?,
            },
            3 => C::QuantityRange {
                minimum: self.optional(Self::quantity)?,
                maximum: self.optional(Self::quantity)?,
                minimum_endpoint: self.endpoint()?,
                maximum_endpoint: self.endpoint()?,
            },
            4 => {
                let negated = self.boolean()?;
                let count = self.count(MAX_MEMBERSHIP_VALUES)?;
                let mut members = Vec::with_capacity(count);
                let mut total = 0_usize;
                for _ in 0..count {
                    let member = self.bytes()?;
                    total = total
                        .checked_add(member.len())
                        .ok_or(Error::InvalidCanonicalEncoding)?;
                    if total > MAX_MEMBERSHIP_BYTES {
                        return Err(Error::MembershipBytesExceeded);
                    }
                    members.push(member);
                }
                C::CanonicalMembership { members, negated }
            }
            5 => {
                let anchored_start = self.boolean()?;
                let anchored_end = self.boolean()?;
                let negated = self.boolean()?;
                let start_state =
                    u16::try_from(self.u32()?).map_err(|_| Error::InvalidCanonicalEncoding)?;
                let maximum_input_characters = self.u32()?;
                let maximum_match_steps = self.u32()?;
                let count = self.count(MAX_PATTERN_STATES)?;
                let mut states = Vec::with_capacity(count);
                let mut total = 0_usize;
                for _ in 0..count {
                    let accepting = self.boolean()?;
                    let count = self.count(MAX_PATTERN_TRANSITIONS)?;
                    total = total
                        .checked_add(count)
                        .ok_or(Error::InvalidCanonicalEncoding)?;
                    if total > MAX_PATTERN_TRANSITIONS {
                        return Err(Error::TooManyPatternTransitions);
                    }
                    let mut transitions = Vec::with_capacity(count);
                    for _ in 0..count {
                        transitions.push(TextPatternTransition {
                            first_scalar: self.u32()?,
                            last_scalar: self.u32()?,
                            target_state: u16::try_from(self.u32()?)
                                .map_err(|_| Error::InvalidCanonicalEncoding)?,
                        });
                    }
                    states.push(TextPatternState {
                        accepting,
                        transitions,
                    });
                }
                C::TextPattern {
                    pattern: CheckedTextPattern {
                        states,
                        start_state,
                        maximum_input_characters,
                        maximum_match_steps,
                    },
                    anchored_start,
                    anchored_end,
                    negated,
                }
            }
            6 => C::FixedIntegerRange {
                minimum: self.optional(Self::bytes)?,
                maximum: self.optional(Self::bytes)?,
                minimum_endpoint: self.endpoint()?,
                maximum_endpoint: self.endpoint()?,
            },
            7 => C::FloatFinite,
            8 => C::FloatRange {
                minimum: self.optional(Self::bytes)?,
                maximum: self.optional(Self::bytes)?,
                minimum_endpoint: self.endpoint()?,
                maximum_endpoint: self.endpoint()?,
            },
            9 => C::NearQuantity {
                source_kind: self.kind()?,
                limit: Box::new(
                    crate::Quantity::decode(&self.bytes()?)
                        .map_err(|_| Error::InvalidCanonicalEncoding)?,
                ),
                schema: self.bytes()?,
            },
            _ => return Err(Error::InvalidCanonicalEncoding),
        })
    }
}

#[cfg(test)]
mod tests;
