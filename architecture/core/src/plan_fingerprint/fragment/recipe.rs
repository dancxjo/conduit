//! Representation of the EXISTING canonical fragment byte stream. No planning,
//! placement selection, authority or semantic validation is performed here.
use super::*;
use crate::canonical_bytes::FragmentScopeRole;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FragmentSealField {
    /// None names the fragment header; Some names original placement ordinal.
    pub placement: Option<u16>,
    pub role: FragmentScopeRole,
    pub entry: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentSealValue<'a> {
    Text(&'a str),
    U64(u64),
}
#[derive(Clone, Copy, Debug)]
pub enum FragmentSealPiece<'a> {
    Literal(&'a [u8]),
    Field(FragmentSealField, bool), // true means canonical length-prefixed text
}
#[derive(Clone, Copy)]
pub struct FragmentSealBinding<'a> {
    pub field: FragmentSealField,
    pub value: FragmentSealValue<'a>,
}
#[derive(Clone, Copy)]
pub struct FragmentSealLimits {
    pub literal_bytes: usize,
    pub pieces: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentSealRefusal {
    Capacity,
    Unsupported,
    Scope,
    Bindings,
}
#[derive(Clone, Copy)]
enum OwnedPiece {
    Literal(usize, usize),
    Field(FragmentSealField, bool),
}
pub struct FragmentSealRecipe {
    literals: Vec<u8>,
    pieces: Vec<OwnedPiece>,
}
impl FragmentSealRecipe {
    /// Builder-side exact measurement, not a resource grant. Uses the same
    /// canonical writer without retaining bytes or allocating any storage.
    pub fn required_limits(
        fragment: &PlanFragment,
    ) -> Result<FragmentSealLimits, FragmentSealRefusal> {
        if !supported(fragment) {
            return Err(FragmentSealRefusal::Unsupported);
        }
        let mut count = Measure {
            bytes: 0,
            pieces: 0,
            literal: false,
            overflow: false,
        };
        emit(&mut count, fragment);
        if count.overflow {
            return Err(FragmentSealRefusal::Capacity);
        }
        Ok(FragmentSealLimits {
            literal_bytes: count.bytes,
            pieces: count.pieces,
        })
    }
    /// Builder-side capture uses the same field serializer as full Core sealing.
    /// All storage is reserved fallibly before encoding. A refused recipe cannot
    /// carry unhandled Boot-dependent tails forward as literal old authority.
    pub fn capture(
        fragment: &PlanFragment,
        limits: FragmentSealLimits,
    ) -> Result<Self, FragmentSealRefusal> {
        if !supported(fragment) {
            return Err(FragmentSealRefusal::Unsupported);
        }
        if fragment.placements.len() > usize::from(u16::MAX)
            || fragment.placements.iter().enumerate().any(|(i, p)| {
                fragment.placements[..i]
                    .iter()
                    .any(|q| q.placement_id == p.placement_id)
            })
        {
            return Err(FragmentSealRefusal::Scope);
        }
        let mut literals = Vec::new();
        let mut pieces = Vec::new();
        literals
            .try_reserve_exact(limits.literal_bytes)
            .map_err(|_| FragmentSealRefusal::Capacity)?;
        pieces
            .try_reserve_exact(limits.pieces)
            .map_err(|_| FragmentSealRefusal::Capacity)?;
        let mut sink = Capture {
            recipe: Self { literals, pieces },
            fragment,
            limits,
            refusal: None,
        };
        emit(&mut sink, fragment);
        if let Some(refusal) = sink.refusal {
            return Err(refusal);
        }
        Ok(sink.recipe)
    }
    pub fn pieces(&self) -> impl Iterator<Item = FragmentSealPiece<'_>> + Clone {
        self.pieces.iter().map(|piece| match *piece {
            OwnedPiece::Literal(start, len) => {
                FragmentSealPiece::Literal(&self.literals[start..start + len])
            }
            OwnedPiece::Field(field, text) => FragmentSealPiece::Field(field, text),
        })
    }
    pub fn digest(
        &self,
        bindings: &[FragmentSealBinding<'_>],
    ) -> Result<[u8; 32], FragmentSealRefusal> {
        digest(self.pieces(), bindings)
    }
    /// Actual container capacities, not semantic resource admission.
    pub fn retained_bytes(&self) -> usize {
        self.literals.capacity() + self.pieces.capacity() * core::mem::size_of::<OwnedPiece>()
    }
}
/// Hash a Make-emitted immutable recipe without allocating or decoding a rich
/// Plan. Exact supplied fields are mandatory; duplicate/missing/extra or wrong
/// value kinds refuse. This result is an identity digest, NEVER Plan approval.
pub fn fragment_seal_recipe_digest(
    pieces: &[FragmentSealPiece<'_>],
    bindings: &[FragmentSealBinding<'_>],
) -> Result<[u8; 32], FragmentSealRefusal> {
    digest(pieces.iter().copied(), bindings)
}
fn digest<'a>(
    pieces: impl Iterator<Item = FragmentSealPiece<'a>> + Clone,
    bindings: &[FragmentSealBinding<'_>],
) -> Result<[u8; 32], FragmentSealRefusal> {
    use FragmentSealRefusal as E;
    for (i, binding) in bindings.iter().enumerate() {
        if bindings[..i].iter().any(|b| b.field == binding.field)
            || !pieces
                .clone()
                .any(|p| matches!(p,FragmentSealPiece::Field(field,_) if field==binding.field))
        {
            return Err(E::Bindings);
        }
    }
    let mut hash = CanonicalHash::new();
    for piece in pieces {
        match piece {
            FragmentSealPiece::Literal(bytes) => hash.extend_from_slice(bytes),
            FragmentSealPiece::Field(field, text) => {
                let value = bindings
                    .iter()
                    .find(|b| b.field == field)
                    .ok_or(E::Bindings)?
                    .value;
                match (text, value) {
                    (true, FragmentSealValue::Text(value))
                        if u32::try_from(value.len()).is_ok() =>
                    {
                        push_string(&mut hash, value)
                    }
                    (false, FragmentSealValue::U64(value)) => push_u64(&mut hash, value),
                    _ => return Err(E::Bindings),
                }
            }
        }
    }
    Ok(hash.finish())
}
fn supported(f: &PlanFragment) -> bool {
    f.realization_backs.is_empty()
        && f.execution_regions.is_empty()
        && f.execution_fusions.is_empty()
        && f.shared_pools.is_empty()
        && f.states.is_empty()
        && f.connections.iter().all(|c| {
            c.resource.is_none() && c.selected_line.is_none() && c.admitted_lines.is_empty()
        })
        && f.fore_ports.iter().all(|p| p.selected_line.is_none())
        && f.placements.iter().all(|p| {
            p.resources
                .iter()
                .all(|r| r.protected.is_none() && r.compute.is_none() && r.content.is_none())
        })
}
struct Capture<'a> {
    recipe: FragmentSealRecipe,
    fragment: &'a PlanFragment,
    limits: FragmentSealLimits,
    refusal: Option<FragmentSealRefusal>,
}
impl Capture<'_> {
    fn field(&mut self, field: Field<'_>, text: bool) {
        if self.refusal.is_some() {
            return;
        }
        let field = match field {
            Field::Header(role) => FragmentSealField {
                placement: None,
                role,
                entry: 0,
            },
            Field::Placement(id, role, entry) => {
                let Some(index) = self
                    .fragment
                    .placements
                    .iter()
                    .position(|p| &p.placement_id == id)
                else {
                    self.refusal = Some(FragmentSealRefusal::Scope);
                    return;
                };
                let Ok(entry) = u16::try_from(entry) else {
                    self.refusal = Some(FragmentSealRefusal::Scope);
                    return;
                };
                FragmentSealField {
                    placement: Some(index as u16),
                    role,
                    entry,
                }
            }
        };
        if self.recipe.pieces.len() == self.limits.pieces {
            self.refusal = Some(FragmentSealRefusal::Capacity);
            return;
        }
        self.recipe.pieces.push(OwnedPiece::Field(field, text));
    }
}
impl CanonicalSink for Capture<'_> {
    fn scope_string(&mut self, field: Field<'_>, _: &str) {
        self.field(field, true);
    }
    fn scope_u64(&mut self, field: Field<'_>, _: u64) {
        self.field(field, false);
    }
    fn push(&mut self, byte: u8) {
        self.extend_from_slice(&[byte]);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        if self.refusal.is_some() || bytes.is_empty() {
            return;
        }
        let start = self.recipe.literals.len();
        if bytes.len() > self.limits.literal_bytes.saturating_sub(start) {
            self.refusal = Some(FragmentSealRefusal::Capacity);
            return;
        }
        if let Some(OwnedPiece::Literal(_, length)) = self.recipe.pieces.last_mut() {
            *length += bytes.len();
        } else {
            if self.recipe.pieces.len() == self.limits.pieces {
                self.refusal = Some(FragmentSealRefusal::Capacity);
                return;
            }
            self.recipe
                .pieces
                .push(OwnedPiece::Literal(start, bytes.len()));
        }
        self.recipe.literals.extend_from_slice(bytes);
    }
}

fn emit(sink: &mut impl CanonicalSink, fragment: &PlanFragment) {
    push_header(sink, fragment.into());
    push_u32(sink, fragment.placements.len() as u32);
    for gear in &fragment.placements {
        placement::push_placement(sink, gear);
    }
    tail::push_tail(sink, fragment.into());
}
struct Measure {
    bytes: usize,
    pieces: usize,
    literal: bool,
    overflow: bool,
}
impl Measure {
    fn next(&mut self) {
        if let Some(n) = self.pieces.checked_add(1) {
            self.pieces = n;
        } else {
            self.overflow = true;
        }
    }
}
impl CanonicalSink for Measure {
    fn scope_string(&mut self, _: Field<'_>, _: &str) {
        self.next();
        self.literal = false;
    }
    fn scope_u64(&mut self, _: Field<'_>, _: u64) {
        self.next();
        self.literal = false;
    }
    fn push(&mut self, _: u8) {
        self.extend_from_slice(&[0]);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if !self.literal {
            self.next();
            self.literal = true;
        }
        if let Some(n) = self.bytes.checked_add(bytes.len()) {
            self.bytes = n;
        } else {
            self.overflow = true;
        }
    }
}
