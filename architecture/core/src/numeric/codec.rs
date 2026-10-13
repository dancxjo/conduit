use super::{symbolic::Node, *};
#[derive(Debug, Clone, Copy)]
pub struct MathValueRef<'a> {
    encoded: &'a [u8],
    facts: Facts,
}
impl<'a> MathValueRef<'a> {
    /// Upper bound for this wire value's decoded owned storage, excluding allocator metadata.
    pub fn owned_storage_upper_bound_bytes(self) -> usize {
        let inline = core::mem::size_of::<MathValue>();
        match self.encoded[1] {
            1 => inline,
            2 => inline + core::mem::size_of::<ExactRational>(),
            3 => {
                let mut cursor = Cursor::new(self.encoded);
                cursor.take(2).expect("validated mathematical header");
                let count = usize::from(cursor.u16().expect("validated symbolic count"));
                let mut bytes = inline
                    + core::mem::size_of::<SymbolicDag>()
                    + count * core::mem::size_of::<Node>();
                for _ in 0..count {
                    let code = cursor.byte().expect("validated symbolic node");
                    if code == 0 {
                        let length = usize::from(cursor.u16().expect("validated literal length"));
                        let literal = cursor.take(length).expect("validated literal");
                        if literal[1] == 2 {
                            bytes += core::mem::size_of::<ExactRational>();
                        }
                    } else {
                        let operation = SymbolicOperation::from_code(
                            code,
                            cursor.byte().expect("validated operation"),
                        )
                        .expect("validated operation code");
                        let length = operation.arity() * core::mem::size_of::<u16>();
                        bytes += length;
                        cursor.take(length).expect("validated operands");
                    }
                }
                bytes
            }
            _ => unreachable!("validated mathematical tag"),
        }
    }
    pub fn validate(encoded: &'a [u8]) -> Result<Self, NumericRefusal> {
        let facts = validate(encoded)?;
        Ok(Self { encoded, facts })
    }
    pub fn is_zero(self) -> bool {
        self.facts.zero
    }
    pub fn proven_nonzero(self) -> bool {
        self.facts.nonzero
    }
    pub fn proven_positive(self) -> bool {
        self.facts.positive
    }
    /// Fixed-capacity rational scratch; unresolved symbolic values stay distinct.
    pub fn exact_rational(self) -> Result<Option<ExactRational>, NumericRefusal> {
        let mut cursor = Cursor::new(self.encoded);
        cursor.byte()?;
        Ok(match cursor.byte()? {
            1 => Some(ExactRational::from_i64(i64::from_le_bytes(
                cursor
                    .take(8)?
                    .try_into()
                    .map_err(|_| NumericRefusal::MalformedEncoding)?,
            ))),
            2 => Some(ExactRational::decode(&mut cursor)?),
            _ => None,
        })
    }
    /// Exact comparison using fixed-capacity rational scratch. Equal canonical
    /// symbolic denotations compare equal; an unproved symbolic ordering is
    /// unknown rather than an approximation or representation ordering.
    pub fn compare(self, other: Self) -> Result<Option<core::cmp::Ordering>, NumericRefusal> {
        if self.encoded == other.encoded {
            return Ok(Some(core::cmp::Ordering::Equal));
        }
        if self.encoded[1] == 1 && other.encoded[1] == 1 {
            return Ok(Some(
                self.checked_i64()
                    .expect("checked small integer")
                    .cmp(&other.checked_i64().expect("checked small integer")),
            ));
        }
        fn rational(value: MathValueRef<'_>) -> Result<Option<ExactRational>, NumericRefusal> {
            let mut cursor = Cursor::new(value.encoded);
            cursor.byte()?;
            Ok(match cursor.byte()? {
                1 => Some(ExactRational::from_i64(i64::from_le_bytes(
                    cursor
                        .take(8)?
                        .try_into()
                        .map_err(|_| NumericRefusal::MalformedEncoding)?,
                ))),
                2 => Some(ExactRational::decode(&mut cursor)?),
                _ => None,
            })
        }
        match (rational(self)?, rational(other)?) {
            (Some(a), Some(b)) => Ok(Some(a.compare(&b)?)),
            _ => Ok(None),
        }
    }
    /// Exact local integer projection. This reads fixed-capacity rational limbs;
    /// it does not materialize an owned mathematical graph or allocate.
    pub fn checked_i64(self) -> Result<i64, crate::NumericProjectionRefusal> {
        self.checked_scaled_i64(1)
    }
    pub fn checked_u64(self) -> Result<u64, crate::NumericProjectionRefusal> {
        if self.encoded[1] == 1 {
            return u64::try_from(self.checked_i64()?)
                .map_err(|_| crate::NumericProjectionRefusal::Overflow);
        }
        let mut cursor = Cursor::new(self.encoded);
        cursor
            .byte()
            .map_err(crate::NumericProjectionRefusal::Budget)?;
        if cursor
            .byte()
            .map_err(crate::NumericProjectionRefusal::Budget)?
            != 2
        {
            return Err(crate::NumericProjectionRefusal::UnresolvedSymbolic);
        }
        let value =
            ExactRational::decode(&mut cursor).map_err(crate::NumericProjectionRefusal::Budget)?;
        if !value.denominator.is_one() {
            return Err(crate::NumericProjectionRefusal::Inexact);
        }
        if value.negative {
            return Err(crate::NumericProjectionRefusal::Overflow);
        }
        value
            .numerator
            .to_u64()
            .ok_or(crate::NumericProjectionRefusal::Overflow)
    }
    /// Explicit floor projection; semantic arithmetic itself remains exact.
    pub fn checked_floor_i64(self) -> Result<i64, crate::NumericProjectionRefusal> {
        let value = self
            .exact_rational()
            .map_err(crate::NumericProjectionRefusal::Budget)?
            .ok_or(crate::NumericProjectionRefusal::UnresolvedSymbolic)?;
        let (mut quotient, remainder) = value
            .numerator
            .div_rem(&value.denominator)
            .map_err(crate::NumericProjectionRefusal::Budget)?;
        if value.negative && !remainder.is_zero() {
            quotient = quotient
                .add(&super::integer::Magnitude::from_u64(1))
                .map_err(crate::NumericProjectionRefusal::Budget)?;
        }
        let integer = ExactRational::new(
            value.negative,
            quotient,
            super::integer::Magnitude::from_u64(1),
        )
        .map_err(crate::NumericProjectionRefusal::Budget)?;
        integer
            .as_i64()
            .ok_or(crate::NumericProjectionRefusal::Overflow)
    }
    pub fn checked_microscalar(
        self,
    ) -> Result<crate::MicroScalar, crate::NumericProjectionRefusal> {
        self.checked_scaled_i64(crate::MicroScalar::SCALE)
            .map(crate::MicroScalar::from_raw_microunits)
    }
    fn checked_scaled_i64(self, scale: i64) -> Result<i64, crate::NumericProjectionRefusal> {
        if self.encoded[1] == 1 {
            let value = i64::from_le_bytes(
                self.encoded[2..10]
                    .try_into()
                    .map_err(|_| crate::NumericProjectionRefusal::Inexact)?,
            );
            return value
                .checked_mul(scale)
                .ok_or(crate::NumericProjectionRefusal::Overflow);
        }
        let mut cursor = Cursor::new(self.encoded);
        cursor
            .byte()
            .map_err(crate::NumericProjectionRefusal::Budget)?;
        let value = match cursor
            .byte()
            .map_err(crate::NumericProjectionRefusal::Budget)?
        {
            1 => ExactRational::from_i64(i64::from_le_bytes(
                cursor
                    .take(8)
                    .map_err(crate::NumericProjectionRefusal::Budget)?
                    .try_into()
                    .map_err(|_| crate::NumericProjectionRefusal::Inexact)?,
            )),
            2 => ExactRational::decode(&mut cursor)
                .map_err(crate::NumericProjectionRefusal::Budget)?,
            _ => return Err(crate::NumericProjectionRefusal::UnresolvedSymbolic),
        };
        let scaled = value
            .checked_mul(&ExactRational::from_i64(scale))
            .map_err(crate::NumericProjectionRefusal::Budget)?;
        if !scaled.denominator.is_one() {
            return Err(crate::NumericProjectionRefusal::Inexact);
        }
        scaled
            .as_i64()
            .ok_or(crate::NumericProjectionRefusal::Overflow)
    }
    /// Copy already admitted wire bytes without allocation or arithmetic.
    pub fn encode_into(self, output: &mut [u8]) -> Result<usize, NumericRefusal> {
        if output.len() < self.encoded.len() {
            return Err(NumericRefusal::OutputTooSmall);
        }
        output[..self.encoded.len()].copy_from_slice(self.encoded);
        Ok(self.encoded.len())
    }
    pub fn shape(self) -> NumericWireShape {
        super::profile::wire_shape(self.encoded)
    }
    pub fn encoded(self) -> &'a [u8] {
        self.encoded
    }
    pub fn to_owned(self) -> Result<MathValue, NumericRefusal> {
        decode(self.encoded)
    }
    pub fn representation_identity(self) -> [u8; 32] {
        crate::semantic_digest(MATH_VALUE_INFO_ID, self.encoded)
    }
}
pub(super) struct Cursor<'a> {
    pub bytes: &'a [u8],
    pub position: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], NumericRefusal> {
        let end = self
            .position
            .checked_add(n)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        self.position = end;
        Ok(slice)
    }
    pub fn byte(&mut self) -> Result<u8, NumericRefusal> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, NumericRefusal> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, NumericRefusal> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}
pub(super) fn node_hash(node: &Node, hashes: &[[u8; 32]]) -> Result<[u8; 32], NumericRefusal> {
    let mut payload = Vec::new();
    match node {
        Node::Literal(value) => {
            payload.push(0);
            payload.extend_from_slice(&encode(value)?);
        }
        Node::Operation { op, operands } => {
            let (code, degree) = op.code();
            payload.extend_from_slice(&[code, degree]);
            for i in operands {
                payload.extend_from_slice(&hashes[usize::from(*i)]);
            }
        }
    }
    Ok(crate::semantic_digest("numeric/node@1", &payload))
}
pub(super) fn key(value: &MathValue) -> Result<[u8; 32], NumericRefusal> {
    match value {
        MathValue::Symbolic(dag) => {
            let mut hashes = [[0; 32]; MAX_SYMBOLIC_NODES];
            for (i, node) in dag.nodes.iter().enumerate() {
                hashes[i] = node_hash(node, &hashes)?;
            }
            Ok(hashes[dag.nodes.len() - 1])
        }
        _ => node_hash(&Node::Literal(value.clone()), &[]),
    }
}
pub(super) fn encode(value: &MathValue) -> Result<Vec<u8>, NumericRefusal> {
    let mut out = Vec::new();
    out.push(1);
    match value {
        MathValue::SmallInteger(n) => {
            out.push(1);
            out.extend_from_slice(&n.to_le_bytes());
        }
        MathValue::Rational(n) => {
            if n.as_i64().is_some() {
                return Err(NumericRefusal::NonCanonical);
            }
            out.push(2);
            n.encode_into(&mut out);
        }
        MathValue::Symbolic(dag) => {
            out.push(3);
            if dag.nodes.is_empty() || dag.nodes.len() > MAX_SYMBOLIC_NODES {
                return Err(NumericRefusal::NodeBudget);
            }
            out.extend_from_slice(&(dag.nodes.len() as u16).to_le_bytes());
            for node in &dag.nodes {
                match node {
                    Node::Literal(v) => {
                        let bytes = encode(v)?;
                        out.push(0);
                        out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
                        out.extend_from_slice(&bytes);
                    }
                    Node::Operation { op, operands } => {
                        let (code, degree) = op.code();
                        out.extend_from_slice(&[code, degree]);
                        for i in operands {
                            out.extend_from_slice(&i.to_le_bytes());
                        }
                    }
                }
            }
        }
    }
    if out.len() > MAX_NUMERIC_ENCODED_BYTES {
        return Err(NumericRefusal::StorageBudget);
    }
    validate(&out)?;
    Ok(out)
}
#[derive(Debug, Clone, Copy, Default)]
struct Facts {
    rational: bool,
    zero: bool,
    one: bool,
    positive: bool,
    nonzero: bool,
    pi_factor: bool,
    inverse_pi_factor: bool,
}
fn atom(cursor: &mut Cursor<'_>) -> Result<Facts, NumericRefusal> {
    if cursor.byte()? != 1 {
        return Err(NumericRefusal::UnsupportedVersion);
    }
    match cursor.byte()? {
        1 => {
            let n = i64::from_le_bytes(cursor.take(8)?.try_into().unwrap());
            Ok(Facts {
                rational: true,
                zero: n == 0,
                one: n == 1,
                positive: n > 0,
                nonzero: n != 0,
                pi_factor: false,
                inverse_pi_factor: false,
            })
        }
        2 => {
            let n = ExactRational::decode(cursor)?;
            if n.as_i64().is_some() {
                return Err(NumericRefusal::NonCanonical);
            }
            Ok(Facts {
                rational: true,
                zero: n.is_zero(),
                one: false,
                positive: !n.is_negative() && !n.is_zero(),
                nonzero: !n.is_zero(),
                pi_factor: false,
                inverse_pi_factor: false,
            })
        }
        _ => Err(NumericRefusal::NonCanonical),
    }
}
fn literal_rational(bytes: &[u8]) -> Result<ExactRational, NumericRefusal> {
    let mut cursor = Cursor::new(bytes);
    cursor.byte()?;
    match cursor.byte()? {
        1 => Ok(ExactRational::from_i64(i64::from_le_bytes(
            cursor.take(8)?.try_into().unwrap(),
        ))),
        2 => ExactRational::decode(&mut cursor),
        _ => Err(NumericRefusal::NonCanonical),
    }
}
fn validate(encoded: &[u8]) -> Result<Facts, NumericRefusal> {
    if encoded.len() > MAX_NUMERIC_ENCODED_BYTES {
        return Err(NumericRefusal::StorageBudget);
    }
    let mut cursor = Cursor::new(encoded);
    if encoded.get(1) != Some(&3) {
        let facts = atom(&mut cursor)?;
        return if cursor.position == encoded.len() {
            Ok(facts)
        } else {
            Err(NumericRefusal::MalformedEncoding)
        };
    }
    if cursor.byte()? != 1 {
        return Err(NumericRefusal::UnsupportedVersion);
    }
    cursor.byte()?;
    let count = usize::from(cursor.u16()?);
    if count == 0 || count > MAX_SYMBOLIC_NODES {
        return Err(NumericRefusal::NodeBudget);
    }
    let mut hashes = [[0; 32]; MAX_SYMBOLIC_NODES];
    let mut depths = [0usize; MAX_SYMBOLIC_NODES];
    let mut facts = [Facts::default(); MAX_SYMBOLIC_NODES];
    let mut children = [[0u16; 2]; MAX_SYMBOLIC_NODES];
    let mut arities = [0usize; MAX_SYMBOLIC_NODES];
    let mut codes = [0u8; MAX_SYMBOLIC_NODES];
    let mut degrees = [0u8; MAX_SYMBOLIC_NODES];
    let mut literals = [None; MAX_SYMBOLIC_NODES];
    for i in 0..count {
        let code = cursor.byte()?;
        codes[i] = code;
        let mut payload = [0u8; 66];
        payload[0] = code;
        let hash = if code == 0 {
            let length = usize::from(cursor.u16()?);
            let bytes = cursor.take(length)?;
            literals[i] = Some(bytes);
            let mut atom_cursor = Cursor::new(bytes);
            facts[i] = atom(&mut atom_cursor)?;
            if atom_cursor.position != bytes.len() {
                return Err(NumericRefusal::MalformedEncoding);
            }
            let mut hash_input = [0u8; 1034];
            if bytes.len() + 1 > hash_input.len() {
                return Err(NumericRefusal::StorageBudget);
            }
            hash_input[0] = 0;
            hash_input[1..bytes.len() + 1].copy_from_slice(bytes);
            depths[i] = 1;
            crate::semantic_digest("numeric/node@1", &hash_input[..bytes.len() + 1])
        } else {
            let degree = cursor.byte()?;
            payload[1] = degree;
            degrees[i] = degree;
            let op = SymbolicOperation::from_code(code, degree)?;
            let arity = op.arity();
            arities[i] = arity;
            let mut depth = 1;
            for j in 0..arity {
                let child = usize::from(cursor.u16()?);
                if child >= i {
                    return Err(NumericRefusal::NonCanonical);
                }
                children[i][j] = child as u16;
                payload[2 + j * 32..2 + (j + 1) * 32].copy_from_slice(&hashes[child]);
                depth = depth.max(depths[child] + 1);
            }
            if op.commutative()
                && hashes[usize::from(children[i][0])] > hashes[usize::from(children[i][1])]
            {
                return Err(NumericRefusal::NonCanonical);
            }
            depths[i] = depth;
            let a = if arity > 0 {
                facts[usize::from(children[i][0])]
            } else {
                Facts::default()
            };
            let b = if arity > 1 {
                facts[usize::from(children[i][1])]
            } else {
                Facts::default()
            };
            facts[i] = match op {
                SymbolicOperation::Pi => Facts {
                    positive: true,
                    nonzero: true,
                    pi_factor: true,
                    ..Facts::default()
                },
                SymbolicOperation::Root(_) => {
                    if a.rational {
                        let rational =
                            literal_rational(literals[usize::from(children[i][0])].unwrap())?;
                        if let Some(n) = rational.as_i64() {
                            if n < 0 && degree % 2 == 0 {
                                return Err(NumericRefusal::Domain);
                            }
                            let magnitude = n.unsigned_abs();
                            let (mut low, mut high) = (0u64, magnitude.min(3_037_000_500));
                            while low < high {
                                let mid = low + (high - low).div_ceil(2);
                                if mid
                                    .checked_pow(u32::from(degree))
                                    .is_some_and(|p| p <= magnitude)
                                {
                                    low = mid;
                                } else {
                                    high = mid - 1;
                                }
                            }
                            if low.checked_pow(u32::from(degree)) == Some(magnitude) {
                                return Err(NumericRefusal::NonCanonical);
                            }
                        }
                    }
                    if a.zero || a.one {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    if !a.positive && !(a.rational && degree % 2 == 1) {
                        return Err(NumericRefusal::IndeterminateDomain);
                    }
                    Facts {
                        positive: a.positive,
                        nonzero: a.nonzero,
                        ..Facts::default()
                    }
                }
                SymbolicOperation::Negate => {
                    if a.rational || a.pi_factor || codes[usize::from(children[i][0])] == 3 {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    Facts {
                        nonzero: a.nonzero,
                        ..Facts::default()
                    }
                }
                SymbolicOperation::Add => {
                    let l = usize::from(children[i][0]);
                    let r = usize::from(children[i][1]);
                    if (codes[l] == 3 && children[l][0] == r as u16)
                        || (codes[r] == 3 && children[r][0] == l as u16)
                    {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    if a.zero
                        || b.zero
                        || (a.rational && b.rational)
                        || (a.pi_factor && b.pi_factor)
                        || (a.inverse_pi_factor && b.inverse_pi_factor)
                    {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    Facts {
                        positive: a.positive && b.positive,
                        nonzero: a.positive && b.positive,
                        ..Facts::default()
                    }
                }
                SymbolicOperation::Multiply => {
                    let l = usize::from(children[i][0]);
                    let r = usize::from(children[i][1]);
                    if (l == r && codes[l] == 2 && degrees[l] == 2)
                        || (codes[l] == 6 && children[l][1] == r as u16 && b.nonzero)
                        || (codes[r] == 6 && children[r][1] == l as u16 && a.nonzero)
                    {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    if a.zero
                        || b.zero
                        || a.one
                        || b.one
                        || (a.rational && b.rational)
                        || (a.pi_factor && b.inverse_pi_factor)
                        || (a.inverse_pi_factor && b.pi_factor)
                        || (a.rational && b.inverse_pi_factor)
                        || (a.inverse_pi_factor && b.rational)
                    {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    Facts {
                        positive: a.positive && b.positive,
                        nonzero: a.nonzero && b.nonzero,
                        pi_factor: (a.rational && b.pi_factor) || (a.pi_factor && b.rational),
                        ..Facts::default()
                    }
                }
                SymbolicOperation::Divide => {
                    if !b.nonzero {
                        return Err(NumericRefusal::IndeterminateDomain);
                    }
                    if a.zero
                        || b.one
                        || (a.rational && b.rational)
                        || (a.pi_factor && b.pi_factor)
                        || (a.inverse_pi_factor && b.inverse_pi_factor)
                        || (a.rational && b.inverse_pi_factor)
                        || (a.inverse_pi_factor && b.rational)
                        || (a.pi_factor && b.rational)
                        || (a.rational && b.pi_factor && codes[usize::from(children[i][1])] != 1)
                        || children[i][0] == children[i][1]
                    {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    Facts {
                        positive: a.positive && b.positive,
                        nonzero: a.nonzero,
                        pi_factor: a.pi_factor && b.rational,
                        inverse_pi_factor: a.rational && b.pi_factor,
                        ..Facts::default()
                    }
                }
                SymbolicOperation::Sin | SymbolicOperation::Cos | SymbolicOperation::Tan => {
                    if a.rational {
                        let rational =
                            literal_rational(literals[usize::from(children[i][0])].unwrap())?;
                        if rational
                            .checked_mul(&ExactRational::from_i64(4))?
                            .as_i64()
                            .is_some()
                        {
                            return Err(NumericRefusal::NonCanonical);
                        }
                    } else if op == SymbolicOperation::Tan {
                        return Err(NumericRefusal::IndeterminateDomain);
                    }
                    Facts::default()
                }
                SymbolicOperation::Asin | SymbolicOperation::Acos => {
                    if !a.rational {
                        return Err(NumericRefusal::IndeterminateDomain);
                    }
                    let rational =
                        literal_rational(literals[usize::from(children[i][0])].unwrap())?;
                    if rational.compare(&ExactRational::from_i64(-1))? == Ordering::Less
                        || rational.compare(&ExactRational::from_i64(1))? == Ordering::Greater
                    {
                        return Err(NumericRefusal::Domain);
                    }
                    if rational.as_i64().is_some() {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    Facts::default()
                }
                SymbolicOperation::Atan => {
                    if a.rational
                        && literal_rational(literals[usize::from(children[i][0])].unwrap())?
                            .as_i64()
                            .is_some_and(|n| (-1..=1).contains(&n))
                    {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    Facts::default()
                }
                SymbolicOperation::Atan2 => {
                    if a.zero || b.zero {
                        return Err(NumericRefusal::NonCanonical);
                    }
                    if !a.nonzero && !b.nonzero {
                        return Err(NumericRefusal::IndeterminateDomain);
                    }
                    Facts::default()
                }
            };
            crate::semantic_digest("numeric/node@1", &payload[..2 + arity * 32])
        };
        if depths[i] > MAX_SYMBOLIC_DEPTH {
            return Err(NumericRefusal::DepthBudget);
        }
        if hashes[..i].contains(&hash) {
            return Err(NumericRefusal::NonCanonical);
        }
        hashes[i] = hash;
    }
    if cursor.position != encoded.len() || encoded.get(4) == Some(&0) && count == 1 {
        return Err(NumericRefusal::NonCanonical);
    }
    let mut seen = [false; MAX_SYMBOLIC_NODES];
    let mut next = 0usize;
    fn visit(
        index: usize,
        children: &[[u16; 2]],
        arities: &[usize],
        seen: &mut [bool],
        next: &mut usize,
    ) -> Result<(), NumericRefusal> {
        if seen[index] {
            return Ok(());
        }
        for child in &children[index][..arities[index]] {
            visit(usize::from(*child), children, arities, seen, next)?;
        }
        if index != *next {
            return Err(NumericRefusal::NonCanonical);
        }
        seen[index] = true;
        *next += 1;
        Ok(())
    }
    visit(count - 1, &children, &arities, &mut seen, &mut next)?;
    if next != count {
        return Err(NumericRefusal::NonCanonical);
    }
    Ok(facts[count - 1])
}
pub(super) fn decode(encoded: &[u8]) -> Result<MathValue, NumericRefusal> {
    validate(encoded)?;
    let mut cursor = Cursor::new(encoded);
    cursor.byte()?;
    match cursor.byte()? {
        1 => Ok(MathValue::integer(i64::from_le_bytes(
            cursor.take(8)?.try_into().unwrap(),
        ))),
        2 => Ok(MathValue::from_rational(ExactRational::decode(
            &mut cursor,
        )?)),
        3 => {
            let count = cursor.u16()?;
            let mut nodes = Vec::with_capacity(usize::from(count));
            for _ in 0..count {
                let code = cursor.byte()?;
                if code == 0 {
                    let len = cursor.u16()?;
                    nodes.push(Node::Literal(decode(cursor.take(usize::from(len))?)?));
                } else {
                    let op = SymbolicOperation::from_code(code, cursor.byte()?)?;
                    let mut operands = Vec::with_capacity(op.arity());
                    for _ in 0..op.arity() {
                        operands.push(cursor.u16()?);
                    }
                    nodes.push(Node::Operation { op, operands });
                }
            }
            Ok(MathValue::Symbolic(Box::new(SymbolicDag { nodes })))
        }
        _ => Err(NumericRefusal::MalformedEncoding),
    }
}
