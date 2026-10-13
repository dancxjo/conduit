//! Unwired concrete four-slot aggregate custody. Production Session extraction,
//! generated initializer wiring and transactional gates are still required.
//! Canonical receipt charges do not bound executor/Native temporary allocation.
use crate::parser_custody_budget::{
    Budget, Cancellation, EnvelopeReservation, Limits, Refusal, Usage,
};
use crate::parser_custody_initialization::PreparedCustodyInitialization;
use crate::parser_model_selection::PreparedParserModelSelection;
use crate::parser_protected_origin::{PreparedProtectedOrigin, ProtectedOriginRefusal};
use crate::parser_retained_commit::{PreparedRetainedCommit, RetainedCommitRefusal};
use crate::parser_transaction_ingress::TransactionIngressRegistry;
use crate::*;
#[path = "parser_session_transaction_delta.rs"]
mod delta;
use alloc::{rc::Rc, sync::Arc, vec::Vec};
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Debug)]
pub(crate) enum SessionTransactionRefusal {
    Resource(Refusal),
    Encoding,
    Allocation,
    Previous,
    Profile,
    Registry,
    Native(conduit_plot::rust_binding::NativeBindingRefusal),
    Origin(ProtectedOriginRefusal),
    Commit(RetainedCommitRefusal),
}
fn size<T: NativeRustBinding + Clone>(v: &T) -> Result<u64, SessionTransactionRefusal> {
    Ok(v.clone()
        .encode()
        .map_err(SessionTransactionRefusal::Native)?
        .len() as u64)
}
fn add(a: u64, b: u64) -> Result<u64, SessionTransactionRefusal> {
    a.checked_add(b).ok_or(SessionTransactionRefusal::Encoding)
}
fn origin_size(v: &PreparedProtectedOrigin) -> Result<u64, SessionTransactionRefusal> {
    let mut n = add(size(v.original())?, size(v.edge())?)?;
    n = add(n, size(v.correlation())?)?;
    n = add(
        n,
        v.program()
            .canonical_bytes()
            .map_err(|_| SessionTransactionRefusal::Encoding)?
            .len() as u64,
    )?;
    add(
        add(n, v.source_input().len() as u64)?,
        v.source_output().len() as u64,
    )
}
fn initialization_charge(
    v: &PreparedCustodyInitialization,
) -> Result<u64, SessionTransactionRefusal> {
    let program_bytes = v
        .program()
        .canonical_bytes()
        .map_err(|_| SessionTransactionRefusal::Encoding)?
        .len() as u64;
    add(
        add(size(v.receipt())?, program_bytes)?,
        add(
            v.source_input().len() as u64,
            v.source_output().len() as u64,
        )?,
    )
}
fn commit_size(
    v: &PreparedRetainedCommit,
    r: &LanguageParserRetainedCommitReceipt,
) -> Result<u64, SessionTransactionRefusal> {
    let mut n = add(size(v.query())?, size(v.anchor())?)?;
    n = add(
        n,
        v.program()
            .canonical_bytes()
            .map_err(|_| SessionTransactionRefusal::Encoding)?
            .len() as u64,
    )?;
    n = add(
        add(n, v.source_input().len() as u64)?,
        v.source_output().len() as u64,
    )?;
    add(n, size(r)?)
}
struct PublishedCustody {
    initialization: PreparedCustodyInitialization,
    protection: LanguageParserProtectedSetProposal,
    beam: Option<LanguageParserJointBeam>,
    facts: Vec<(
        LanguageParserStableDependencyAdmission,
        LanguageParserRetainedFactReceipt,
    )>,
    origins: Vec<PreparedProtectedOrigin>,
    insertions: Vec<LanguageParserProtectedInsertReceipt>,
    rebases: Vec<LanguageParserProtectedRebaseReceipt>,
    commits: Vec<(PreparedRetainedCommit, LanguageParserRetainedCommitReceipt)>,
    snapshots: Vec<LanguageParserRetainedSnapshotReceipt>,
}
pub(crate) struct SessionTransactionCustody {
    model: Arc<PreparedParserModelSelection>,
    published: PublishedCustody,
    budget: Budget,
    ingresses: Rc<TransactionIngressRegistry>,
}
/// Exact maxima are admitted before any consuming Source/model port is invoked.
/// Additional counts bound one staged publication, not an executor heap.
pub(crate) struct TransactionEnvelope {
    pub maximum_next: Usage,
    pub maximum_candidate_bytes: u64,
}
impl SessionTransactionCustody {
    pub(crate) fn prepare(
        model: Arc<PreparedParserModelSelection>,
        limits: Limits,
        initialization: PreparedCustodyInitialization,
        ingresses: Rc<TransactionIngressRegistry>,
    ) -> Result<Self, SessionTransactionRefusal> {
        let protection = initialization.receipt().output().clone();
        let bytes = add(
            add(initialization_charge(&initialization)?, size(&protection)?)?,
            delta::model_charge(&model)?,
        )?;
        let budget = Budget::new(
            limits,
            Usage {
                bytes,
                ..Usage::default()
            },
        )
        .map_err(SessionTransactionRefusal::Resource)?;
        ingresses.seal();
        Ok(Self {
            model,
            published: PublishedCustody {
                initialization,
                protection,
                beam: None,
                facts: Vec::new(),
                origins: Vec::new(),
                insertions: Vec::new(),
                rebases: Vec::new(),
                commits: Vec::new(),
                snapshots: Vec::new(),
            },
            budget,
            ingresses,
        })
    }
    pub(crate) fn cancel(&self) {
        self.budget.cancellation().cancel();
        self.ingresses.close();
    }
    pub(crate) fn protection(&self) -> &LanguageParserProtectedSetProposal {
        &self.published.protection
    }
    pub(crate) fn usage(&self) -> Usage {
        self.budget.used()
    }
    pub(crate) fn stage(
        &mut self,
        envelope: TransactionEnvelope,
    ) -> Result<SessionTransactionStage<'_>, SessionTransactionRefusal> {
        if self.ingresses.closed() {
            return Err(SessionTransactionRefusal::Registry);
        }
        let old = self.budget.used();
        let cancellation = self.budget.cancellation();
        let reservation = self
            .budget
            .reserve_envelope(envelope.maximum_next, envelope.maximum_candidate_bytes)
            .map_err(SessionTransactionRefusal::Resource)?;
        // Reserve every retained destination before exposing a consuming lease.
        macro_rules! reserve {
            ($field:ident,$counter:ident) => {
                self.published
                    .$field
                    .try_reserve_exact((envelope.maximum_next.$counter - old.$counter) as usize)
                    .map_err(|_| SessionTransactionRefusal::Allocation)?;
            };
        }
        reserve!(facts, facts);
        reserve!(origins, origins);
        reserve!(insertions, origins);
        reserve!(rebases, rebases);
        reserve!(commits, commits);
        reserve!(snapshots, snapshots);
        let delta = StagedCustody::new(&self.published, old, envelope.maximum_next)?;
        Ok(SessionTransactionStage {
            destination: &mut self.published,
            delta,
            reservation: Some(reservation),
            old,
            model: &self.model,
            ingresses: self.ingresses.clone(),
            cancellation,
            consumed: false,
            published: false,
            refused: false,
        })
    }
}
struct StagedCustody {
    protection: LanguageParserProtectedSetProposal,
    beam: Option<LanguageParserJointBeam>,
    facts: Vec<(
        LanguageParserStableDependencyAdmission,
        LanguageParserRetainedFactReceipt,
    )>,
    origins: Vec<PreparedProtectedOrigin>,
    insertions: Vec<LanguageParserProtectedInsertReceipt>,
    rebases: Vec<LanguageParserProtectedRebaseReceipt>,
    commits: Vec<(PreparedRetainedCommit, LanguageParserRetainedCommitReceipt)>,
    snapshots: Vec<LanguageParserRetainedSnapshotReceipt>,
    ceiling: Usage,
}
impl StagedCustody {
    fn new(
        old: &PublishedCustody,
        used: Usage,
        ceiling: Usage,
    ) -> Result<Self, SessionTransactionRefusal> {
        let mut s = Self {
            protection: old.protection.clone(),
            beam: old.beam.clone(),
            facts: Vec::new(),
            origins: Vec::new(),
            insertions: Vec::new(),
            rebases: Vec::new(),
            commits: Vec::new(),
            snapshots: Vec::new(),
            ceiling,
        };
        macro_rules! reserve {
            ($field:ident,$counter:ident) => {
                s.$field
                    .try_reserve_exact((ceiling.$counter - used.$counter) as usize)
                    .map_err(|_| SessionTransactionRefusal::Allocation)?;
            };
        }
        reserve!(facts, facts);
        reserve!(origins, origins);
        reserve!(insertions, origins);
        reserve!(rebases, rebases);
        reserve!(commits, commits);
        reserve!(snapshots, snapshots);
        Ok(s)
    }
}
/// One publication owns the entire staged delta and exclusive destination.
/// All registered ports close automatically if consumed work is abandoned.
pub(crate) struct SessionTransactionStage<'a> {
    destination: &'a mut PublishedCustody,
    delta: StagedCustody,
    reservation: Option<EnvelopeReservation<'a>>,
    old: Usage,
    model: &'a Arc<PreparedParserModelSelection>,
    ingresses: Rc<TransactionIngressRegistry>,
    cancellation: Cancellation,
    consumed: bool,
    published: bool,
    refused: bool,
}
impl SessionTransactionStage<'_> {
    fn abort(&self) {
        self.cancellation.cancel();
        self.ingresses.close();
    }
    pub(crate) fn before_ingress(
        &mut self,
        registry: &Rc<TransactionIngressRegistry>,
    ) -> Result<(), SessionTransactionRefusal> {
        let result = if self.refused || !Rc::ptr_eq(registry, &self.ingresses) || registry.closed()
        {
            Err(SessionTransactionRefusal::Registry)
        } else {
            self.reservation
                .as_ref()
                .expect("unpublished reservation")
                .check_before_consumption()
                .map_err(SessionTransactionRefusal::Resource)
        };
        if result.is_err() {
            self.refused = true;
            if self.consumed {
                self.abort();
            }
        }
        result
    }
    pub(crate) fn record_ingress(&mut self, consumed: bool, failed: bool) {
        self.consumed |= consumed;
        self.refused |= failed;
        if failed && self.consumed {
            self.abort();
        }
    }
    fn admission_check(&mut self) -> Result<(), SessionTransactionRefusal> {
        if self.refused || self.ingresses.closed() {
            return Err(SessionTransactionRefusal::Registry);
        }
        let result = self
            .reservation
            .as_ref()
            .expect("unpublished reservation")
            .check_before_consumption()
            .map_err(SessionTransactionRefusal::Resource);
        if result.is_err() {
            self.refused = true;
            if self.consumed {
                self.abort();
            }
        }
        result
    }
    fn admitted<T>(
        &mut self,
        result: Result<T, SessionTransactionRefusal>,
    ) -> Result<T, SessionTransactionRefusal> {
        if result.is_err() {
            self.refused = true;
            if self.consumed {
                self.abort();
            }
        }
        result
    }
    pub(crate) fn retain_fact(
        &mut self,
        admission: LanguageParserStableDependencyAdmission,
    ) -> Result<(), SessionTransactionRefusal> {
        self.admission_check()?;
        let result = self.retain_fact_inner(admission);
        self.admitted(result)
    }
    pub(crate) fn acquire(
        &mut self,
        origin: LanguageParserIndependentProtectedAdmission,
    ) -> Result<(), SessionTransactionRefusal> {
        self.admission_check()?;
        let result = self.acquire_inner(origin);
        self.admitted(result)
    }
    pub(crate) fn rebase(
        &mut self,
        receipt: LanguageParserProtectedRebaseReceipt,
    ) -> Result<(), SessionTransactionRefusal> {
        self.admission_check()?;
        let result = self.rebase_inner(receipt);
        self.admitted(result)
    }
    pub(crate) fn commit(
        &mut self,
        query: LanguageParserJointCommitQuery,
        next: LanguageParserJointBeam,
    ) -> Result<(), SessionTransactionRefusal> {
        self.admission_check()?;
        let result = self.commit_inner(query, next);
        self.admitted(result)
    }
    pub(crate) fn snapshot(
        &mut self,
        receipt: LanguageParserRetainedSnapshotReceipt,
    ) -> Result<(), SessionTransactionRefusal> {
        self.admission_check()?;
        let result = self.snapshot_inner(receipt);
        self.admitted(result)
    }
    pub(crate) fn publish(mut self) -> Result<(), SessionTransactionRefusal> {
        self.admission_check()?;
        let (usage, candidate) = self.charges()?;
        self.reservation
            .take()
            .expect("unpublished reservation")
            .publish_exact(usage, candidate)
            .map_err(SessionTransactionRefusal::Resource)?;
        // Destination capacities and every complete receipt were prepared first.
        self.destination.facts.append(&mut self.delta.facts);
        self.destination.origins.append(&mut self.delta.origins);
        self.destination
            .insertions
            .append(&mut self.delta.insertions);
        self.destination.rebases.append(&mut self.delta.rebases);
        self.destination.commits.append(&mut self.delta.commits);
        self.destination.snapshots.append(&mut self.delta.snapshots);
        core::mem::swap(&mut self.destination.protection, &mut self.delta.protection);
        core::mem::swap(&mut self.destination.beam, &mut self.delta.beam);
        self.published = true;
        Ok(())
    }
}
impl Drop for SessionTransactionStage<'_> {
    fn drop(&mut self) {
        if self.consumed && !self.published {
            self.abort();
        }
    }
}
