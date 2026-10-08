//! Unwired concrete protected-set custody draft. Generated Source prerequisites
//! and typed positive/negative tests are required before production integration.
//! Prepared candidate byte charges are canonical evidence charges, not a bound
//! on temporary Native decoding or executor allocation.
use crate::parser_custody_budget::{
    Budget, Cancellation, EnvelopeReservation, Limits, Refusal, Usage,
};
use crate::parser_protected_origin::{PreparedProtectedOrigin, ProtectedOriginRefusal};
use crate::{
    LanguageParserProtectedInitialReceipt, LanguageParserProtectedInsertReceipt,
    LanguageParserProtectedOriginCorrelation, LanguageParserProtectedRebaseReceipt,
    LanguageParserProtectedSetProposal,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

pub(crate) enum CustodyRefusal {
    Origin(ProtectedOriginRefusal),
    Native(NativeBindingRefusal),
    Resource(Refusal),
    Previous,
    Allocation,
    Encoding,
}
impl core::fmt::Debug for CustodyRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Origin(v) => f.debug_tuple("Origin").field(v).finish(),
            Self::Native(v) => f.debug_tuple("Native").field(v).finish(),
            Self::Resource(v) => f.debug_tuple("Resource").field(v).finish(),
            Self::Previous => f.write_str("Previous"),
            Self::Allocation => f.write_str("Allocation"),
            Self::Encoding => f.write_str("Encoding"),
        }
    }
}
impl From<ProtectedOriginRefusal> for CustodyRefusal {
    fn from(v: ProtectedOriginRefusal) -> Self {
        Self::Origin(v)
    }
}
impl From<Refusal> for CustodyRefusal {
    fn from(v: Refusal) -> Self {
        Self::Resource(v)
    }
}
/// Exact full origins and each full lineage receipt survive every publication.
/// This owner does not authorize a parser fact or playback acknowledgement.
pub(crate) struct ProtectedCustody {
    current: LanguageParserProtectedSetProposal,
    origins: Vec<PreparedProtectedOrigin>,
    initial: LanguageParserProtectedInitialReceipt,
    insertions: Vec<LanguageParserProtectedInsertReceipt>,
    rebases: Vec<LanguageParserProtectedRebaseReceipt>,
    budget: Budget,
}
fn encoded_size<T: NativeRustBinding + Clone>(v: &T) -> Result<u64, CustodyRefusal> {
    let s = v
        .clone()
        .into_structured()
        .map_err(|_| CustodyRefusal::Encoding)?;
    let b = s.canonical_bytes().map_err(|_| CustodyRefusal::Encoding)?;
    u64::try_from(b.len()).map_err(|_| CustodyRefusal::Encoding)
}
fn origin_size(v: &PreparedProtectedOrigin) -> Result<u64, CustodyRefusal> {
    // Retain exact original and Source input/output; original and input are
    // deliberately both charged because both immutable bodies are owned.
    let program_bytes = v
        .program()
        .canonical_bytes()
        .map_err(|_| CustodyRefusal::Encoding)?;
    encoded_size(v.original())?
        .checked_add(program_bytes.len() as u64)
        .and_then(|n| n.checked_add(encoded_size(v.edge()).ok()?))
        .and_then(|n| n.checked_add(v.source_input().len() as u64))
        .and_then(|n| n.checked_add(v.source_output().len() as u64))
        .and_then(|n| n.checked_add(encoded_size(v.correlation()).ok()?))
        .ok_or(CustodyRefusal::Encoding)
}
impl ProtectedCustody {
    pub(crate) fn prepare(
        limits: Limits,
        origin: PreparedProtectedOrigin,
    ) -> Result<Self, CustodyRefusal> {
        let initial = origin.initial()?;
        let current = origin.original().output().clone();
        let bytes = origin_size(&origin)?
            .checked_add(encoded_size(&current)?)
            .and_then(|n| n.checked_add(encoded_size(&initial).ok()?))
            .ok_or(CustodyRefusal::Encoding)?;
        let budget = Budget::new(
            limits,
            Usage {
                origins: 1,
                bytes,
                ..Usage::default()
            },
        )?;
        let mut origins = Vec::new();
        origins
            .try_reserve_exact(4)
            .map_err(|_| CustodyRefusal::Allocation)?;
        origins.push(origin);
        let mut insertions = Vec::new();
        insertions
            .try_reserve_exact(4)
            .map_err(|_| CustodyRefusal::Allocation)?;
        Ok(Self {
            current,
            origins,
            initial,
            insertions,
            rebases: Vec::new(),
            budget,
        })
    }
    pub(crate) fn initial(&self) -> &LanguageParserProtectedInitialReceipt {
        &self.initial
    }
    pub(crate) fn cancellation(&self) -> Cancellation {
        self.budget.cancellation()
    }
    pub(crate) fn current(&self) -> &LanguageParserProtectedSetProposal {
        &self.current
    }
    pub(crate) fn acquire(
        &mut self,
        origin: PreparedProtectedOrigin,
    ) -> Result<(), CustodyRefusal> {
        let insertion = origin.insertion(&self.current)?;
        let next = insertion.output().clone();
        // These native constructors execute the complete Source correlation
        // laws. Host custody does not decide graph preservation.
        for prior in &self.origins {
            LanguageParserProtectedOriginCorrelation::new(next.clone(), prior.edge().clone())
                .map_err(CustodyRefusal::Native)?;
        }
        let owned = origin_size(&origin)?
            .checked_add(encoded_size(&insertion)?)
            .ok_or(CustodyRefusal::Encoding)?;
        let old = self.budget.used();
        let next_bytes = old
            .bytes
            .checked_sub(encoded_size(&self.current)?)
            .and_then(|n| n.checked_add(owned))
            .and_then(|n| n.checked_add(encoded_size(&next).ok()?))
            .ok_or(CustodyRefusal::Encoding)?;
        let usage = Usage {
            origins: old.origins.checked_add(1).ok_or(CustodyRefusal::Encoding)?,
            bytes: next_bytes,
            ..old
        };
        let reservation = self.budget.reserve(
            usage,
            owned
                .checked_add(encoded_size(&next)?)
                .ok_or(CustodyRefusal::Encoding)?,
        )?;
        // Capacity was reserved for all four slots at initialization.
        reservation.publish()?;
        self.origins.push(origin);
        self.insertions.push(insertion);
        self.current = next;
        Ok(())
    }
    /// Reserve before the caller consumes the exact Source rebase input.
    /// Complete actual retained charges are validated within the pre-reserved
    /// envelope before publication; executor allocations are separately owned.
    pub(crate) fn stage_rebase(
        &mut self,
        envelope: u64,
    ) -> Result<ProtectedRebaseStage<'_>, CustodyRefusal> {
        if envelope == 0 {
            return Err(CustodyRefusal::Resource(Refusal::Limits));
        }
        let old = self.budget.used();
        let next = Usage {
            rebases: old
                .rebases
                .checked_add(1)
                .ok_or(CustodyRefusal::Resource(Refusal::Pressure))?,
            bytes: old
                .bytes
                .checked_add(envelope)
                .ok_or(CustodyRefusal::Resource(Refusal::Pressure))?,
            ..old
        };
        let reservation = self.budget.reserve_envelope(next, envelope)?;
        self.rebases
            .try_reserve_exact(1)
            .map_err(|_| CustodyRefusal::Allocation)?;
        Ok(ProtectedRebaseStage {
            current: &mut self.current,
            origins: &self.origins,
            rebases: &mut self.rebases,
            reservation,
            previous_usage: old,
        })
    }
    pub(crate) fn rebase(
        &mut self,
        receipt: LanguageParserProtectedRebaseReceipt,
    ) -> Result<(), CustodyRefusal> {
        if receipt.context().previous() != &self.current {
            return Err(CustodyRefusal::Previous);
        }
        let next = receipt.output().clone();
        for origin in &self.origins {
            LanguageParserProtectedOriginCorrelation::new(next.clone(), origin.edge().clone())
                .map_err(CustodyRefusal::Native)?;
        }
        let charge = encoded_size(&receipt)?;
        let old = self.budget.used();
        let next_bytes = old
            .bytes
            .checked_sub(encoded_size(&self.current)?)
            .and_then(|n| n.checked_add(charge))
            .and_then(|n| n.checked_add(encoded_size(&next).ok()?))
            .ok_or(CustodyRefusal::Encoding)?;
        let usage = Usage {
            rebases: old.rebases.checked_add(1).ok_or(CustodyRefusal::Encoding)?,
            bytes: next_bytes,
            ..old
        };
        let reservation = self.budget.reserve(
            usage,
            charge
                .checked_add(encoded_size(&next)?)
                .ok_or(CustodyRefusal::Encoding)?,
        )?;
        self.rebases
            .try_reserve_exact(1)
            .map_err(|_| CustodyRefusal::Allocation)?;
        reservation.publish()?;
        self.rebases.push(receipt);
        self.current = next;
        Ok(())
    }
}

/// One exclusive prepared transaction. No executor is invoked until the Session
/// obtains this stage. Late refusal leaves published custody intact, but the
/// Session must poison an ingress that already consumed its input.
pub(crate) struct ProtectedRebaseStage<'a> {
    current: &'a mut LanguageParserProtectedSetProposal,
    origins: &'a [PreparedProtectedOrigin],
    rebases: &'a mut Vec<LanguageParserProtectedRebaseReceipt>,
    reservation: EnvelopeReservation<'a>,
    previous_usage: Usage,
}
impl ProtectedRebaseStage<'_> {
    pub(crate) fn check_before_consumption(&self) -> Result<(), CustodyRefusal> {
        Ok(self.reservation.check_before_consumption()?)
    }
    pub(crate) fn finish(
        self,
        receipt: LanguageParserProtectedRebaseReceipt,
    ) -> Result<(), CustodyRefusal> {
        if receipt.context().previous() != &*self.current {
            return Err(CustodyRefusal::Previous);
        }
        let next = receipt.output().clone();
        for origin in self.origins {
            LanguageParserProtectedOriginCorrelation::new(next.clone(), origin.edge().clone())
                .map_err(CustodyRefusal::Native)?;
        }
        let actual = encoded_size(&receipt)?
            .checked_add(encoded_size(&next)?)
            .ok_or(CustodyRefusal::Encoding)?;
        let next_bytes = self
            .previous_usage
            .bytes
            .checked_sub(encoded_size(&*self.current)?)
            .and_then(|n| n.checked_add(actual))
            .ok_or(CustodyRefusal::Encoding)?;
        let usage = Usage {
            rebases: self
                .previous_usage
                .rebases
                .checked_add(1)
                .ok_or(CustodyRefusal::Encoding)?,
            bytes: next_bytes,
            ..self.previous_usage
        };
        // Capacity and envelope were reserved before ingress. Publication below
        // moves already-prepared bodies and cannot execute another Source step.
        self.reservation.publish_exact(usage, actual)?;
        self.rebases.push(receipt);
        *self.current = next;
        Ok(())
    }
}
