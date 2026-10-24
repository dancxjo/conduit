//! Root-private saved user state. Not yet wired to IRQ capture or native entry.
//! The caller must provide actual pre-normalization selectors/bases and retain
//! the admitted mapping lease. This module grants neither admission nor fuel.
use crate::protected_region::RegionBinding;
use conduit_core::OperationSlice;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub(super) struct UserRegisters {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub(super) struct UserReturn {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub(super) struct UserSegments {
    pub ds: u16,
    pub es: u16,
    pub fs: u16,
    pub gs: u16,
    pub fs_base: u64,
    pub gs_base: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C, align(16))]
pub(super) struct UserContext {
    pub registers: UserRegisters,
    pub returned: UserReturn,
    pub segments: UserSegments,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    InvalidMapping,
    ForeignOwner,
    InvalidFrame,
    UnsupportedState,
    AlreadyCaptured,
    Consumed,
    Empty,
    Revoked,
}
impl UserContext {
    /// Exact entry_return.S source-IRQ saved layout:15GPRs,tag,5IRET words.
    /// These words must be copied while the actual interrupt stack is live.
    /// Selectors and MSR bases must be captured separately BEFORE normalization.
    pub(super) fn from_source_irq(
        words: &[u64; 21],
        segments: UserSegments,
    ) -> Result<Self, Refusal> {
        if words[15] != 0 {
            return Err(Refusal::InvalidFrame);
        }
        Ok(Self {
            registers: UserRegisters {
                r15: words[0],
                r14: words[1],
                r13: words[2],
                r12: words[3],
                r11: words[4],
                r10: words[5],
                r9: words[6],
                r8: words[7],
                rdi: words[8],
                rsi: words[9],
                rbp: words[10],
                rbx: words[11],
                rdx: words[12],
                rcx: words[13],
                rax: words[14],
            },
            returned: UserReturn {
                rip: words[16],
                cs: words[17],
                rflags: words[18],
                rsp: words[19],
                ss: words[20],
            },
            segments,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExecutableRange {
    pub start: u64,
    pub end: u64,
}
/// Immutable metadata supplied from the actual installed AddressSpace, not from
/// a guest packet. It must remain borrowed from that live admitted owner.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Mapping<'a> {
    pub instance: u64,
    pub cr3: u64,
    pub image_sha256: [u8; 32],
    pub stack_start: u64,
    pub stack_end: u64,
    pub executable: &'a [ExecutableRange],
}
impl Mapping<'_> {
    fn valid(&self) -> bool {
        self.instance != 0
            && self.cr3 != 0
            && self.cr3 & 4095 == 0
            && self.stack_start < self.stack_end
            && user_address(self.stack_end)
            && !self.executable.is_empty()
            && self
                .executable
                .iter()
                .all(|r| r.start < r.end && user_address(r.end))
            && self.executable.windows(2).all(|r| r[0].end <= r[1].start)
    }
    fn validate(&self, context: &UserContext) -> Result<(), Refusal> {
        let r = context.returned;
        if !self
            .executable
            .iter()
            .any(|x| x.start <= r.rip && r.rip < x.end)
            || r.rsp < self.stack_start
            || r.rsp > self.stack_end
        {
            return Err(Refusal::InvalidFrame);
        }
        // Fixed GDT user long code/data. Arithmetic flags, DF, AC, RF and ID
        // survive; TF,NT,IOPL,VM and reserved bits are unsupported. IF stays on.
        const ALLOWED: u64 = 0x0000_0000_0025_0ed7;
        if r.cs != 0x2b || r.ss != 0x33 || r.rflags & !ALLOWED != 0 || r.rflags & 0x202 != 0x202 {
            return Err(Refusal::UnsupportedState);
        }
        let s = context.segments;
        // Preserve null selector RPL and legal readable code/data selectors.
        // Current GDT has no LDT or user base descriptors. Null hidden bases
        // are observed explicitly: never infer a zero base from selector alone.
        if ![s.ds, s.es, s.fs, s.gs].into_iter().all(data_selector)
            || s.fs_base != 0
            || s.gs_base != 0
        {
            return Err(Refusal::UnsupportedState);
        }
        Ok(())
    }
}
fn user_address(address: u64) -> bool {
    address < (1_u64 << 47)
}
fn data_selector(selector: u16) -> bool {
    selector < 4 || (selector & 4 == 0 && matches!(selector >> 3, 5 | 6))
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Empty,
    Captured,
    Consumed,
    Revoked,
}
/// Single live mapping's custody, subordinate to existing operation accounting.
/// Captured slice is the OLD interrupted slice. A later new slice's admission
/// and fuel are checked by ProtectedOperation/owner, not invented here.
pub(super) struct Custody<'a, E: Copy + Eq> {
    binding: &'a RegionBinding,
    mapping: &'a Mapping<'a>,
    envelope: E,
    state: State,
    saved: Option<UserContext>,
    captured_slice: Option<OperationSlice<E>>,
}
impl<'a, E: Copy + Eq> Custody<'a, E> {
    pub(super) fn new(
        binding: &'a RegionBinding,
        mapping: &'a Mapping<'a>,
        envelope: E,
    ) -> Result<Self, Refusal> {
        if !mapping.valid() {
            return Err(Refusal::InvalidMapping);
        }
        Ok(Self {
            binding,
            mapping,
            envelope,
            state: State::Empty,
            saved: None,
            captured_slice: None,
        })
    }
    fn owner(
        &self,
        binding: &RegionBinding,
        mapping: &Mapping<'_>,
        envelope: E,
    ) -> Result<(), Refusal> {
        if self.binding != binding || self.mapping != mapping || self.envelope != envelope {
            return Err(Refusal::ForeignOwner);
        }
        if self.state == State::Revoked {
            return Err(Refusal::Revoked);
        }
        Ok(())
    }
    pub(super) fn capture(
        &mut self,
        binding: &RegionBinding,
        mapping: &Mapping<'_>,
        envelope: E,
        slice: OperationSlice<E>,
        context: UserContext,
    ) -> Result<(), Refusal> {
        self.owner(binding, mapping, envelope)?;
        if self.state == State::Captured {
            return Err(Refusal::AlreadyCaptured);
        }
        if self.captured_slice == Some(slice) {
            return Err(Refusal::Consumed);
        }
        self.mapping.validate(&context)?;
        self.saved = Some(context);
        self.captured_slice = Some(slice);
        self.state = State::Captured;
        Ok(())
    }
    /// Consume once using the exact interrupted slice identity, NOT its new
    /// resume grant. Parent validates that new grant on the same operation.
    pub(super) fn consume(
        &mut self,
        binding: &RegionBinding,
        mapping: &Mapping<'_>,
        envelope: E,
        captured_slice: OperationSlice<E>,
    ) -> Result<UserContext, Refusal> {
        self.owner(binding, mapping, envelope)?;
        if self.state == State::Empty {
            return Err(Refusal::Empty);
        }
        if self.state == State::Consumed {
            return Err(Refusal::Consumed);
        }
        if self.captured_slice != Some(captured_slice) {
            return Err(Refusal::ForeignOwner);
        }
        let context = self.saved.ok_or(Refusal::Empty)?;
        self.mapping.validate(&context)?;
        self.saved = None;
        self.state = State::Consumed;
        Ok(context)
    }
    pub(super) fn revoke(&mut self) {
        self.saved = None;
        self.captured_slice = None;
        self.state = State::Revoked;
    }
}
const _: () = assert!(core::mem::size_of::<UserRegisters>() == 120);
const _: () = assert!(core::mem::size_of::<UserReturn>() == 40);
const _: () = assert!(core::mem::size_of::<UserContext>() == 192);
const _: () = assert!(core::mem::align_of::<UserContext>() == 16);
#[cfg(test)]
#[path = "continuation/tests.rs"]
mod tests;
