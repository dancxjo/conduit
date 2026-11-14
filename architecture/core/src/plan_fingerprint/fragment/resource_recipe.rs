//! Match the existing primitive resource serializer at its typed recipe slots.
//! This checks static bytes; it grants no resource possession or admission.
use super::recipe::{FragmentSealPiece as P, FragmentSealRefusal};
use crate::canonical_bytes::{CanonicalSink, FragmentScopeField, FragmentScopeRole as R};

/// Require exact class/units/content contract/Base/residence bytes from the
/// signed recipe. Pool and content owner identities remain separately checked
/// typed substitutions. Protected/compute resources are outside this profile.
pub fn fragment_seal_resource_matches(
    pieces: &[P<'_>],
    placement: u16,
    resource: u16,
    offer: &crate::ResourceOffer,
    units: u32,
) -> Result<(), FragmentSealRefusal> {
    if offer.compute.is_some() || units == 0 || units > offer.capacity_units {
        return Err(FragmentSealRefusal::Scope);
    }
    let mut starts = pieces.iter().enumerate().filter_map(|(i, p)| match p {
        P::Field(f, true)
            if f.placement == Some(placement)
                && f.entry == resource
                && f.role == R::ResourcePool =>
        {
            Some(i)
        }
        _ => None,
    });
    let start = starts.next().ok_or(FragmentSealRefusal::Scope)?;
    if starts.next().is_some() {
        return Err(FragmentSealRefusal::Scope);
    }
    let mut check = Check {
        pieces: &pieces[start..],
        index: 0,
        offset: 0,
        placement,
        resource,
        valid: true,
    };
    crate::resource_canonical::push_offer_binding(&mut check, offer, units);
    if check.valid {
        Ok(())
    } else {
        Err(FragmentSealRefusal::Scope)
    }
}
struct Check<'a, 'b> {
    pieces: &'a [P<'b>],
    index: usize,
    offset: usize,
    placement: u16,
    resource: u16,
    valid: bool,
}
impl CanonicalSink for Check<'_, '_> {
    fn push(&mut self, byte: u8) {
        self.extend_from_slice(&[byte]);
    }
    fn extend_from_slice(&mut self, mut bytes: &[u8]) {
        while self.valid && !bytes.is_empty() {
            let Some(P::Literal(literal)) = self.pieces.get(self.index) else {
                self.valid = false;
                return;
            };
            let remaining = &literal[self.offset..];
            let n = remaining.len().min(bytes.len());
            if remaining[..n] != bytes[..n] {
                self.valid = false;
                return;
            }
            self.offset += n;
            bytes = &bytes[n..];
            if self.offset == literal.len() {
                self.index += 1;
                self.offset = 0;
            }
        }
    }
    fn scope_string(&mut self, field: FragmentScopeField<'_>, _: &str) {
        let FragmentScopeField::Header(role) = field else {
            self.valid = false;
            return;
        };
        if self.offset != 0
            || !matches!(self.pieces.get(self.index), Some(P::Field(f, true)) if f.placement == Some(self.placement) && f.entry == self.resource && f.role == role)
        {
            self.valid = false;
            return;
        }
        self.index += 1;
    }
}
