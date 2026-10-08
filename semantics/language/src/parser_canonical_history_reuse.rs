//! Representation reuse only: this cannot admit Native values or authorize publication.
//! The sealed driver owns `C`, binding the complete immutable Source/bank, revision,
//! model selection, basis and history book. `H` remains its opaque replay parent.
//! All histories and the exact-Type validator remain owned and charged by the book.
//! Immutable borrows prevent this cache from surviving book mutation/publication.
use conduit_core::{
    validate_canonical_structured_value, PreparedStructuredValueValidator, StructuredInfoRefusal,
    ValidatedCanonicalStructuredValue,
};
use core::mem::size_of;

/// Borrowed representation of an opaque parent owned by the sealed driver.
/// Implementing this view supplies no Source, Native or commitment authority.
pub trait CanonicalHistory<C> {
    fn context_owner(&self) -> &C;
    fn canonical_raw_state(&self) -> &[u8];
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal<E> {
    Capacity,
    ForeignContext,
    Basis,
    Value(StructuredInfoRefusal),
    Replay(E),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Storage {
    pub retained_inline_bytes: usize,
    pub construction_inline_overlap_bytes: usize,
}
/// Four immutable parent borrows scoped to one completed, unpublished epoch.
/// `C` must be the actual driver context retaining every original Source/bank,
/// revision, selection, basis and book owner; equal identifiers are insufficient.
/// Returning a parent requires the caller's original replay to succeed.
pub struct Reuse<'a, C, H> {
    context: &'a C,
    validator: &'a PreparedStructuredValueValidator,
    basis: ValidatedCanonicalStructuredValue<'a>,
    parents: [Option<&'a H>; 4],
    next: usize,
}
impl<'a, C, H: CanonicalHistory<C>> Reuse<'a, C, H> {
    /// No allocation. The caller separately admits validator/history storage and
    /// full Source/Native replay work; those are never covered by this receipt.
    pub fn prepare(
        context: &'a C,
        validator: &'a PreparedStructuredValueValidator,
        basis: &'a [u8],
        maximum_inline: usize,
        maximum_overlap: usize,
    ) -> Result<Self, Refusal<()>> {
        let receipt = Self::storage();
        if receipt.retained_inline_bytes > maximum_inline
            || receipt.construction_inline_overlap_bytes > maximum_overlap
        {
            return Err(Refusal::Capacity);
        }
        let basis = validate_canonical_structured_value(basis).map_err(Refusal::Value)?;
        Ok(Self {
            context,
            validator,
            basis,
            parents: [None; 4],
            next: 0,
        })
    }
    pub const fn storage() -> Storage {
        Storage {
            retained_inline_bytes: size_of::<Self>(),
            construction_inline_overlap_bytes: 2 * size_of::<Self>(),
        }
    }
    fn check<E>(&self, context: &C, state: &[u8]) -> Result<(), Refusal<E>> {
        if !core::ptr::eq(context, self.context) {
            return Err(Refusal::ForeignContext);
        }
        self.validator.validate(state).map_err(Refusal::Value)?;
        let state = validate_canonical_structured_value(state).map_err(Refusal::Value)?;
        if state.record_field("basis").map_err(Refusal::Value)? != Some(self.basis) {
            return Err(Refusal::Basis);
        }
        Ok(())
    }
    /// The mandatory callback must replay the actual retained parent through
    /// original Source and full Native admission before this representation is reused.
    pub fn remember<E>(
        &mut self,
        context: &C,
        parent: &'a H,
        replay: impl FnOnce(&H) -> Result<(), E>,
    ) -> Result<(), Refusal<E>> {
        self.check(context, parent.canonical_raw_state())?;
        if !core::ptr::eq(parent.context_owner(), self.context) {
            return Err(Refusal::ForeignContext);
        }
        replay(parent).map_err(Refusal::Replay)?;
        self.parents[self.next] = Some(parent);
        self.next = (self.next + 1) % 4;
        Ok(())
    }
    pub fn lookup<E>(
        &self,
        context: &C,
        state: &[u8],
        mut replay: impl FnMut(&H) -> Result<(), E>,
    ) -> Result<Option<&'a H>, Refusal<E>> {
        self.check(context, state)?;
        for parent in self.parents.iter().flatten() {
            self.check(parent.context_owner(), parent.canonical_raw_state())?;
            if parent.canonical_raw_state() == state {
                replay(parent).map_err(Refusal::Replay)?;
                return Ok(Some(*parent));
            }
        }
        Ok(None)
    }
}
