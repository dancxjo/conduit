//! Intrinsic portable admission bounds, independent of approximation requirements.
use super::*;
use alloc::string::String;
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NumericForms(u8);
impl NumericForms {
    pub const INTEGER: Self = Self(1);
    pub const RATIONAL: Self = Self(2);
    pub const SYMBOLIC: Self = Self(4);
    pub const ALL: Self = Self(7);
    pub const fn bits(self) -> u8 {
        self.0
    }
    pub fn from_bits(bits: u8) -> Result<Self, NumericRefusal> {
        if bits == 0 || bits & !7 != 0 {
            return Err(NumericRefusal::InvalidProfile);
        }
        Ok(Self(bits))
    }
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PortableNumericProfile {
    pub forms: NumericForms,
    pub maximum_integer_limbs: u16,
    pub maximum_symbolic_nodes: u16,
    pub maximum_symbolic_depth: u8,
    pub maximum_encoded_bytes: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NumericWireShape {
    pub form: NumericForms,
    pub maximum_integer_limbs: u16,
    pub symbolic_nodes: u16,
    pub symbolic_depth: u8,
    pub encoded_bytes: u32,
}
impl PortableNumericProfile {
    pub const MAXIMAL: Self = Self {
        forms: NumericForms::ALL,
        maximum_integer_limbs: MAX_INTEGER_LIMBS as u16,
        maximum_symbolic_nodes: MAX_SYMBOLIC_NODES as u16,
        maximum_symbolic_depth: MAX_SYMBOLIC_DEPTH as u8,
        maximum_encoded_bytes: MAX_NUMERIC_ENCODED_BYTES as u32,
    };
    pub const ENCODED_LEN: usize = 10;
    pub fn encode(self) -> Result<[u8; Self::ENCODED_LEN], NumericRefusal> {
        self.checked()?;
        let mut bytes = [0; Self::ENCODED_LEN];
        bytes[0] = self.forms.bits();
        bytes[1..3].copy_from_slice(&self.maximum_integer_limbs.to_le_bytes());
        bytes[3..5].copy_from_slice(&self.maximum_symbolic_nodes.to_le_bytes());
        bytes[5] = self.maximum_symbolic_depth;
        bytes[6..10].copy_from_slice(&self.maximum_encoded_bytes.to_le_bytes());
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, NumericRefusal> {
        if bytes.len() != Self::ENCODED_LEN {
            return Err(NumericRefusal::MalformedEncoding);
        }
        Self {
            forms: NumericForms::from_bits(bytes[0])?,
            maximum_integer_limbs: u16::from_le_bytes([bytes[1], bytes[2]]),
            maximum_symbolic_nodes: u16::from_le_bytes([bytes[3], bytes[4]]),
            maximum_symbolic_depth: bytes[5],
            maximum_encoded_bytes: u32::from_le_bytes(
                bytes[6..10]
                    .try_into()
                    .map_err(|_| NumericRefusal::MalformedEncoding)?,
            ),
        }
        .checked()
    }
    pub fn for_value(value: MathValueRef<'_>) -> Self {
        let shape = value.shape();
        Self {
            forms: shape.form,
            maximum_integer_limbs: shape.maximum_integer_limbs.max(1),
            maximum_symbolic_nodes: shape.symbolic_nodes,
            maximum_symbolic_depth: shape.symbolic_depth,
            maximum_encoded_bytes: shape.encoded_bytes.max(10),
        }
    }
    pub fn union(self, other: Self) -> Result<Self, NumericRefusal> {
        Self {
            forms: self.forms.union(other.forms),
            maximum_integer_limbs: self.maximum_integer_limbs.max(other.maximum_integer_limbs),
            maximum_symbolic_nodes: self
                .maximum_symbolic_nodes
                .max(other.maximum_symbolic_nodes),
            maximum_symbolic_depth: self
                .maximum_symbolic_depth
                .max(other.maximum_symbolic_depth),
            maximum_encoded_bytes: self.maximum_encoded_bytes.max(other.maximum_encoded_bytes),
        }
        .checked()
    }
    pub fn admits_profile(self, actual: Self) -> bool {
        self.checked().is_ok()
            && actual.checked().is_ok()
            && self.forms.contains(actual.forms)
            && self.maximum_integer_limbs >= actual.maximum_integer_limbs
            && self.maximum_symbolic_nodes >= actual.maximum_symbolic_nodes
            && self.maximum_symbolic_depth >= actual.maximum_symbolic_depth
            && self.maximum_encoded_bytes >= actual.maximum_encoded_bytes
    }
    pub fn checked(self) -> Result<Self, NumericRefusal> {
        if self.forms.0 == 0
            || self.forms.0 & !7 != 0
            || self.maximum_integer_limbs == 0
            || usize::from(self.maximum_integer_limbs) > MAX_INTEGER_LIMBS
            || usize::from(self.maximum_symbolic_nodes) > MAX_SYMBOLIC_NODES
            || usize::from(self.maximum_symbolic_depth) > MAX_SYMBOLIC_DEPTH
            || self.maximum_encoded_bytes < 10
            || self.maximum_encoded_bytes as usize > MAX_NUMERIC_ENCODED_BYTES
            || (self.forms.contains(NumericForms::SYMBOLIC)
                && (self.maximum_symbolic_nodes == 0 || self.maximum_symbolic_depth == 0))
        {
            return Err(NumericRefusal::InvalidProfile);
        }
        Ok(self)
    }
    pub fn info_id(self) -> Result<String, NumericRefusal> {
        self.checked()?;
        Ok(alloc::format!(
            "numeric/profile-{}-{}-{}-{}-{}@1",
            self.forms.0,
            self.maximum_integer_limbs,
            self.maximum_symbolic_nodes,
            self.maximum_symbolic_depth,
            self.maximum_encoded_bytes
        ))
    }
    pub fn from_info_id(id: &str) -> Option<Self> {
        let body = id.strip_prefix("numeric/profile-")?.strip_suffix("@1")?;
        let mut fields = body.split('-');
        fn number(fields: &mut core::str::Split<'_, char>) -> Option<u32> {
            let text = fields.next()?;
            if text.is_empty()
                || (text.len() > 1 && text.starts_with('0'))
                || !text.bytes().all(|c| c.is_ascii_digit())
            {
                return None;
            }
            text.parse().ok()
        }
        let forms = u8::try_from(number(&mut fields)?).ok()?;
        let limbs = u16::try_from(number(&mut fields)?).ok()?;
        let nodes = u16::try_from(number(&mut fields)?).ok()?;
        let depth = u8::try_from(number(&mut fields)?).ok()?;
        let bytes = number(&mut fields)?;
        if fields.next().is_some() {
            return None;
        }
        Self {
            forms: NumericForms(forms),
            maximum_integer_limbs: limbs,
            maximum_symbolic_nodes: nodes,
            maximum_symbolic_depth: depth,
            maximum_encoded_bytes: bytes,
        }
        .checked()
        .ok()
    }
    pub fn validate<'a>(self, encoded: &'a [u8]) -> Result<MathValueRef<'a>, NumericRefusal> {
        self.checked()?;
        if encoded.len() > self.maximum_encoded_bytes as usize {
            return Err(NumericRefusal::ProfileViolation);
        }
        let value = MathValueRef::validate(encoded)?;
        let shape = value.shape();
        if !self.forms.contains(shape.form)
            || shape.maximum_integer_limbs > self.maximum_integer_limbs
            || shape.symbolic_nodes > self.maximum_symbolic_nodes
            || shape.symbolic_depth > self.maximum_symbolic_depth
        {
            return Err(NumericRefusal::ProfileViolation);
        }
        Ok(value)
    }
}
pub(super) fn wire_shape(encoded: &[u8]) -> NumericWireShape {
    fn limbs(encoded: &[u8]) -> u16 {
        if encoded[1] == 1 {
            let n = i64::from_le_bytes(encoded[2..10].try_into().unwrap()).unsigned_abs();
            return if n == 0 {
                0
            } else if n < 1_000_000_000 {
                1
            } else if n < 1_000_000_000_000_000_000 {
                2
            } else {
                3
            };
        }
        let numerator = u16::from_le_bytes(encoded[3..5].try_into().unwrap());
        let pos = 5 + usize::from(numerator) * 4;
        let denominator = u16::from_le_bytes(encoded[pos..pos + 2].try_into().unwrap());
        numerator.max(denominator)
    }
    if encoded[1] != 3 {
        return NumericWireShape {
            form: if encoded[1] == 1 || {
                let numerator = usize::from(u16::from_le_bytes(encoded[3..5].try_into().unwrap()));
                let pos = 5 + numerator * 4;
                u16::from_le_bytes(encoded[pos..pos + 2].try_into().unwrap()) == 1
                    && u32::from_le_bytes(encoded[pos + 2..pos + 6].try_into().unwrap()) == 1
            } {
                NumericForms::INTEGER
            } else {
                NumericForms::RATIONAL
            },
            maximum_integer_limbs: limbs(encoded),
            symbolic_nodes: 0,
            symbolic_depth: 0,
            encoded_bytes: encoded.len() as u32,
        };
    }
    let mut cursor = super::codec::Cursor::new(encoded);
    cursor.position = 2;
    let count = cursor.u16().unwrap();
    let mut depths = [0u8; MAX_SYMBOLIC_NODES];
    let mut max_limbs = 0;
    let mut depth = 0;
    for i in 0..usize::from(count) {
        let code = cursor.byte().unwrap();
        let node_depth = if code == 0 {
            let len = cursor.u16().unwrap();
            let literal = cursor.take(usize::from(len)).unwrap();
            max_limbs = max_limbs.max(limbs(literal));
            1
        } else {
            let op = SymbolicOperation::from_code(code, cursor.byte().unwrap()).unwrap();
            let mut current = 1;
            for _ in 0..op.arity() {
                current = current.max(depths[usize::from(cursor.u16().unwrap())] + 1);
            }
            current
        };
        depths[i] = node_depth;
        depth = depth.max(node_depth);
    }
    NumericWireShape {
        form: NumericForms::SYMBOLIC,
        maximum_integer_limbs: max_limbs,
        symbolic_nodes: count,
        symbolic_depth: depth,
        encoded_bytes: encoded.len() as u32,
    }
}
