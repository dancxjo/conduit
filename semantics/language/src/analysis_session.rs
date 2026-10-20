//! One shared dependency-analysis authority over the existing revision journal.
//! Installed owners retain this session; receipt preparation alone is not current.
use crate::dependency_commitment::CommittedSnapshot;
use crate::{dependency_commitment::PreparedCommittedDependency, revision::*, *};
use alloc::vec::Vec;
use conduit_core::revision::*;
#[derive(Debug)]
pub enum AnalysisSessionRefusal {
    Revision(RevisionRefusal),
    Cancelled,
    Pressure,
    Stale,
    Commitment,
    Corrected,
}
pub struct AnalysisSession<'a> {
    journal: RevisionJournal<'a, DependencyRevisions<'a>>,
    source: &'a LanguageText,
    // Retain every prior snapshot through execution: replacement cannot drop
    // the final share of a full Native admission or lexical tape.
    committed: Vec<CommittedSnapshot>,
    publication_dispute: Option<RevisionReference<'a>>,
}
/// Borrowing the owning session prevents advancing it while using this authority.
/// This is analysis commitment, not delivery or playback authority.
pub struct CurrentCommittedDependency<'s, 'a> {
    session: &'s AnalysisSession<'a>,
    reference: RevisionReference<'a>,
    receipt: &'s PreparedCommittedDependency,
}
impl CurrentCommittedDependency<'_, '_> {
    pub fn receipt(&self) -> &PreparedCommittedDependency {
        self.receipt
    }
    pub fn reference(&self) -> RevisionReference<'_> {
        self.reference
    }
    pub fn frontier(&self) -> TokenFrontier {
        self.session.journal.frontiers().committed
    }
}
impl<'a> AnalysisSession<'a> {
    pub fn new(
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<Self, RevisionRefusal> {
        Ok(Self {
            journal: RevisionJournal::new(domain, context, TokenFrontier(0), limits)?,
            source: domain.source_text,
            committed: Vec::with_capacity(limits.history_events),
            publication_dispute: None,
        })
    }
    pub(crate) fn from_retained_prefix(
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
        initial: TokenFrontier,
    ) -> Result<Self, RevisionRefusal> {
        Ok(Self {
            journal: RevisionJournal::new(domain, context, initial, limits)?,
            source: domain.source_text,
            committed: Vec::with_capacity(limits.history_events),
            publication_dispute: None,
        })
    }
    pub(crate) fn context(&self) -> RevisionContext<'a> {
        self.journal.context()
    }
    fn current_arc(
        &self,
        reference: RevisionReference<'a>,
    ) -> Result<&LanguageDependencyArc, AnalysisSessionRefusal> {
        if self.journal.current_proposal() != Some(reference) {
            return Err(AnalysisSessionRefusal::Stale);
        }
        let event = self
            .journal
            .history()
            .find(|e| e.reference() == reference)
            .ok_or(AnalysisSessionRefusal::Stale)?;
        match event.change() {
            RevisionChange::Proposed {
                delta: DependencyDelta::Assert(arc),
            }
            | RevisionChange::Revised {
                delta: DependencyDelta::Assert(arc),
                ..
            } => Ok(arc),
            _ => Err(AnalysisSessionRefusal::Stale),
        }
    }
    pub fn append(
        &mut self,
        event: &'a RevisionEvent<'a, DependencyRevisions<'a>>,
        receipt: Option<&PreparedCommittedDependency>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<(), AnalysisSessionRefusal> {
        use AnalysisSessionRefusal::*;
        if cancelled {
            return Err(Cancelled);
        }
        if pressure {
            return Err(Pressure);
        }
        if let RevisionChange::Committed { revision, through } = event.change() {
            if self.publication_dispute.is_some() {
                return Err(Corrected);
            }
            if self.committed.len() == self.committed.capacity() {
                return Err(Pressure);
            }
            let receipt = receipt.ok_or(Commitment)?;
            let arc = self.current_arc(*revision)?;
            let query = receipt.admission().query();
            if query.source().material() != self.source
                || arc != receipt.arc()
                || through.0 != *receipt.commitment().result().frontier().committed()
                || self.journal.frontiers().committed.0
                    != *query.commitment().frontier().committed()
                || self
                    .journal
                    .frontiers()
                    .stable_through
                    .is_none_or(|stable| *through > stable)
            {
                return Err(Commitment);
            }
        }
        self.journal.append(event).map_err(Revision)?;
        if matches!(event.change(), RevisionChange::Corrected { .. }) {
            // Keep the committed history and receipts, but do not grant new
            // publication from disputed analysis without checked reconciliation.
            self.publication_dispute = Some(event.reference());
        }
        if matches!(event.change(), RevisionChange::Committed { .. }) {
            let receipt = receipt.ok_or(Commitment)?;
            self.committed.push(receipt.shared_snapshot());
        }
        Ok(())
    }
    pub fn current<'s>(
        &'s self,
        reference: RevisionReference<'a>,
        receipt: &'s PreparedCommittedDependency,
    ) -> Result<CurrentCommittedDependency<'s, 'a>, AnalysisSessionRefusal> {
        if self.publication_dispute.is_some() {
            return Err(AnalysisSessionRefusal::Corrected);
        }
        if !self
            .committed
            .last()
            .is_some_and(|snapshot| snapshot.matches(receipt))
            || receipt.admission().query().source().material() != self.source
            || self.current_arc(reference)? != receipt.arc()
            || self.journal.frontiers().committed.0 <= *receipt.arc().dependent().token().ordinal()
            || self.journal.frontiers().committed.0
                != *receipt.commitment().result().frontier().committed()
        {
            return Err(AnalysisSessionRefusal::Commitment);
        }
        Ok(CurrentCommittedDependency {
            session: self,
            reference,
            receipt,
        })
    }
    pub fn frontiers(&self) -> RevisionFrontiers<TokenFrontier> {
        self.journal.frontiers()
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.journal.is_closed()
    }
    pub fn publication_dispute(&self) -> Option<RevisionReference<'a>> {
        self.publication_dispute
    }
    pub fn history(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a RevisionEvent<'a, DependencyRevisions<'a>>> + '_ {
        self.journal.history()
    }
}
