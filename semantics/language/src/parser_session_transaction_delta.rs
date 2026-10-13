//! Source-admitted concrete aggregate delta operations; not a parser policy.
use super::*;
impl SessionTransactionStage<'_> {
    pub(super) fn seed_inner(
        &mut self,
        execution: SeedExecution,
    ) -> Result<(), SessionTransactionRefusal> {
        if self.delta.beam.is_some()
            || self.destination.seed.is_some()
            || self.delta.seed.is_some()
            || execution.entry() != ParserSessionEntry::Seed
            || execution.input().begin().basis() != self.delta.protection.basis()
            || execution.input().lexical().tape().profile() != self.model.expected_lexical_profile()
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        let input = execution.input();
        let raw = execution.output();
        // Only metadata assembly: every candidate is the unchanged Source output.
        // Source owns initial state, activity, lexical choices and scores.
        let beam = LanguageParserJointBeam::new(
            input.begin().basis().clone(),
            admit_seed_candidate(raw.candidate0())?,
            admit_seed_candidate(raw.candidate1())?,
            admit_seed_candidate(raw.candidate2())?,
            admit_seed_candidate(raw.candidate3())?,
            *input.lexical().tape().source().sequence(),
            execution.ordinal(),
            input.lexical().clone(),
        )
        .map_err(SessionTransactionRefusal::Native)?;
        self.delta.seed = Some(execution);
        self.delta.beam = Some(beam);
        Ok(())
    }

    fn fact_capacity(&self) -> Result<(), SessionTransactionRefusal> {
        if u64::from(self.old.facts) + self.delta.facts.len() as u64
            >= u64::from(self.delta.ceiling.facts)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
        Ok(())
    }
    pub(super) fn retain_fact_inner(
        &mut self,
        execution: StableFactExecution,
        admission: LanguageParserStableDependencyAdmission,
    ) -> Result<(), SessionTransactionRefusal> {
        self.fact_capacity()?;
        if execution.entry() != ParserSessionEntry::StableFact
            || execution.input() != admission.fact().query()
            || execution.output().query() != execution.input()
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        let beam = admission.fact().query().beam();
        if beam.lexical().tape().profile() != self.model.expected_lexical_profile() {
            return Err(SessionTransactionRefusal::Profile);
        }
        if beam.basis() != self.delta.protection.basis() {
            return Err(SessionTransactionRefusal::Previous);
        }
        if self.delta.beam.as_ref() != Some(beam) {
            return Err(SessionTransactionRefusal::Previous);
        }
        let receipt =
            LanguageParserRetainedFactReceipt::new(beam.clone(), admission.fact().clone())
                .map_err(SessionTransactionRefusal::Native)?;
        self.delta.facts.push(RetainedExecutedFact {
            admission,
            receipt,
            execution,
        });
        Ok(())
    }
    pub(super) fn acquire_inner(
        &mut self,
        execution: ProtectedInsertExecution,
        original: LanguageParserIndependentProtectedAdmission,
    ) -> Result<(), SessionTransactionRefusal> {
        if usize::from(self.old.origins) + self.delta.origins.len()
            >= usize::from(self.delta.ceiling.origins)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
        if execution.entry() != ParserSessionEntry::ProtectedInsert
            || execution.input() != original.insert()
            || !self
                .destination
                .facts
                .iter()
                .chain(self.delta.facts.iter())
                .any(|fact| fact.admission.fact() == original.admission().fact())
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        if original.insert().previous() != &self.delta.protection {
            return Err(SessionTransactionRefusal::Previous);
        }
        if self.delta.beam.as_ref() != Some(original.admission().fact().query().beam()) {
            return Err(SessionTransactionRefusal::Previous);
        }
        let origin = PreparedProtectedOrigin::prepare(original)
            .map_err(SessionTransactionRefusal::Origin)?;
        let receipt = origin
            .insertion(&self.delta.protection)
            .map_err(SessionTransactionRefusal::Origin)?;
        if receipt.output() != execution.output() {
            return Err(SessionTransactionRefusal::Previous);
        }
        let next = receipt.output().clone();
        for prior in self
            .destination
            .origins
            .iter()
            .chain(self.delta.origins.iter())
        {
            if prior.edge().dependent() == origin.edge().dependent() {
                return Err(SessionTransactionRefusal::Previous);
            }
            LanguageParserProtectedOriginCorrelation::new(next.clone(), prior.edge().clone())
                .map_err(SessionTransactionRefusal::Native)?;
        }
        self.delta.origins.push(origin);
        self.delta
            .insertions
            .push(RetainedExecutedInsertion { receipt, execution });
        self.delta.protection = next;
        Ok(())
    }
    pub(super) fn rebase_inner(
        &mut self,
        execution: ProtectedRebaseExecution,
    ) -> Result<(), SessionTransactionRefusal> {
        if u64::from(self.old.rebases) + self.delta.rebases.len() as u64
            >= u64::from(self.delta.ceiling.rebases)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
        if execution.entry() != ParserSessionEntry::ProtectedRebase {
            return Err(SessionTransactionRefusal::Previous);
        }
        let beam = self
            .delta
            .beam
            .as_ref()
            .ok_or(SessionTransactionRefusal::Previous)?;
        if execution.input().rebase().previous().state() != beam.candidate0().parser().state()
            || execution.input().rebase().previous().lexical().tape() != beam.lexical().tape()
            || execution
                .input()
                .rebase()
                .previous()
                .lexical()
                .token_count()
                != beam.lexical().token_count()
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        let receipt = LanguageParserProtectedRebaseReceipt::new(
            execution.input().clone(),
            execution.output().clone(),
        )
        .map_err(SessionTransactionRefusal::Native)?;
        if receipt.context().previous() != &self.delta.protection {
            return Err(SessionTransactionRefusal::Previous);
        }
        for origin in self
            .destination
            .origins
            .iter()
            .chain(self.delta.origins.iter())
        {
            LanguageParserProtectedOriginCorrelation::new(
                receipt.output().clone(),
                origin.edge().clone(),
            )
            .map_err(SessionTransactionRefusal::Native)?;
        }
        self.delta.protection = receipt.output().clone();
        self.delta
            .rebases
            .push(RetainedExecutedRebase { receipt, execution });
        Ok(())
    }
    pub(super) fn commit_inner(
        &mut self,
        execution: CommitExecution,
    ) -> Result<(), SessionTransactionRefusal> {
        if u64::from(self.old.commits) + self.delta.commits.len() as u64
            >= u64::from(self.delta.ceiling.commits)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
        if execution.entry() != ParserSessionEntry::Commit
            || !self
                .destination
                .facts
                .iter()
                .chain(self.delta.facts.iter())
                .any(|fact| fact.admission.fact() == execution.input().fact())
            || self.delta.beam.as_ref() != Some(execution.input().fact().query().beam())
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        let previous = execution.input().fact().query().beam();
        let raw = execution.output();
        let next = LanguageParserJointBeam::new(
            previous.basis().clone(),
            admit_raw_candidate(raw.candidate0())?,
            admit_raw_candidate(raw.candidate1())?,
            admit_raw_candidate(raw.candidate2())?,
            admit_raw_candidate(raw.candidate3())?,
            *previous.epoch(),
            *previous.invocation(),
            previous.lexical().clone(),
        )
        .map_err(SessionTransactionRefusal::Native)?;
        let prepared = PreparedRetainedCommit::prepare(execution.input().clone())
            .map_err(SessionTransactionRefusal::Commit)?;
        let receipt = prepared
            .admit_output(next.clone())
            .map_err(SessionTransactionRefusal::Commit)?;
        self.delta.commits.push(RetainedExecutedCommit {
            anchor: prepared,
            receipt,
            execution,
        });
        self.delta.beam = Some(next);
        Ok(())
    }
    pub(super) fn snapshot_inner(
        &mut self,
        receipt: LanguageParserRetainedSnapshotReceipt,
    ) -> Result<(), SessionTransactionRefusal> {
        if u64::from(self.old.snapshots) + self.delta.snapshots.len() as u64
            >= u64::from(self.delta.ceiling.snapshots)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
        if receipt.beam().basis() != self.delta.protection.basis()
            || receipt.beam().lexical().tape().profile() != self.model.expected_lexical_profile()
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        if self.delta.beam.is_none() {
            return Err(SessionTransactionRefusal::Previous);
        }
        // Until a Source-owned complete beam advance witness is integrated,
        // forbid replacing published/staged graph or committed state through
        // an otherwise individually valid snapshot. No Rust legality shortcut.
        if self
            .delta
            .beam
            .as_ref()
            .is_some_and(|beam| beam != receipt.beam())
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        self.delta.beam = Some(receipt.beam().clone());
        self.delta.snapshots.push(receipt);
        Ok(())
    }
    pub(super) fn charges(&self) -> Result<(Usage, u64), SessionTransactionRefusal> {
        // A protected rebase and its parser beam must publish together; mixed
        // revision custody can never become the next Session publication.
        if self
            .delta
            .beam
            .as_ref()
            .is_some_and(|beam| beam.basis() != self.delta.protection.basis())
        {
            return Err(SessionTransactionRefusal::Previous);
        }
        let mut candidate = add(
            size(&self.delta.protection)?,
            self.delta.beam.as_ref().map(size).transpose()?.unwrap_or(0),
        )?;
        if let Some(seed) = &self.delta.seed {
            candidate = add(candidate, add(size(seed.input())?, size(seed.output())?)?)?;
            candidate = add(
                candidate,
                add(
                    seed.input_bytes().len() as u64,
                    seed.output_bytes().len() as u64,
                )?,
            )?;
        }
        for fact in &self.delta.facts {
            candidate = add(
                candidate,
                add(size(&fact.admission)?, size(&fact.receipt)?)?,
            )?;
            candidate = add(
                candidate,
                add(
                    size(fact.execution.input())?,
                    size(fact.execution.output())?,
                )?,
            )?;
            candidate = add(
                candidate,
                add(
                    fact.execution.input_bytes().len() as u64,
                    fact.execution.output_bytes().len() as u64,
                )?,
            )?;
        }
        for origin in &self.delta.origins {
            candidate = add(candidate, origin_size(origin)?)?;
        }
        for insertion in &self.delta.insertions {
            candidate = add(candidate, size(&insertion.receipt)?)?;
            candidate = add(
                candidate,
                add(
                    size(insertion.execution.input())?,
                    size(insertion.execution.output())?,
                )?,
            )?;
            candidate = add(
                candidate,
                add(
                    insertion.execution.input_bytes().len() as u64,
                    insertion.execution.output_bytes().len() as u64,
                )?,
            )?;
        }
        for rebased in &self.delta.rebases {
            candidate = add(candidate, size(&rebased.receipt)?)?;
            candidate = add(
                candidate,
                add(
                    size(rebased.execution.input())?,
                    size(rebased.execution.output())?,
                )?,
            )?;
            candidate = add(
                candidate,
                add(
                    rebased.execution.input_bytes().len() as u64,
                    rebased.execution.output_bytes().len() as u64,
                )?,
            )?;
        }
        for committed in &self.delta.commits {
            candidate = add(
                candidate,
                commit_size(&committed.anchor, &committed.receipt)?,
            )?;
            candidate = add(
                candidate,
                add(
                    size(committed.execution.input())?,
                    size(committed.execution.output())?,
                )?,
            )?;
            candidate = add(
                candidate,
                add(
                    committed.execution.input_bytes().len() as u64,
                    committed.execution.output_bytes().len() as u64,
                )?,
            )?;
        }
        for receipt in &self.delta.snapshots {
            candidate = add(candidate, size(receipt)?)?;
        }
        let old_current = add(
            size(&self.destination.protection)?,
            self.destination
                .beam
                .as_ref()
                .map(size)
                .transpose()?
                .unwrap_or(0),
        )?;
        let bytes = add(
            self.old
                .bytes
                .checked_sub(old_current)
                .ok_or(SessionTransactionRefusal::Encoding)?,
            candidate,
        )?;
        Ok((
            Usage {
                origins: self.old.origins + self.delta.origins.len() as u8,
                facts: self.old.facts + self.delta.facts.len() as u32,
                rebases: self.old.rebases + self.delta.rebases.len() as u32,
                commits: self.old.commits + self.delta.commits.len() as u32,
                snapshots: self.old.snapshots + self.delta.snapshots.len() as u32,
                bytes,
            },
            candidate,
        ))
    }
}

// Charge complete retained model bodies and descriptor material; separately
// prepared numerical executor storage remains under its target's admission.
pub(super) fn model_charge(
    model: &PreparedParserModelSelection,
) -> Result<u64, SessionTransactionRefusal> {
    fn artifact_charge(a: &conduit_ai::ModelArtifact) -> Result<u64, SessionTransactionRefusal> {
        let texts = add(
            add(
                a.architecture_profile.len() as u64,
                a.format_profile.len() as u64,
            )?,
            a.precision_profile.len() as u64,
        )?;
        let reference = a
            .content
            .encode()
            .map_err(|_| SessionTransactionRefusal::Encoding)?;
        add(add(texts, 3 * 4 + 4 + 32)?, reference.len() as u64)
    }
    let resource = model.prepared_categorical().resource();
    let mut n = add(
        resource.bytes().len() as u64,
        artifact_charge(resource.artifact())?,
    )?;
    n = add(n, size(resource.signature())?)?;
    n = add(n, size(model.expected_lexical_profile())?)?;
    n = add(n, 7 * 32 + 3 * 8)?; // Exact compatibility fields, including score/shape bounds.
    if let Some(declaration) = model.declaration() {
        n = add(n, size(declaration.lexical_profile())?)?;
        n = add(n, size(declaration.provenance())?)?;
        n = add(n, artifact_charge(declaration.artifact())?)?;
        n = add(n, size(declaration.signature())?)?;
        n = add(n, declaration.identity().len() as u64)?;
        n = add(n, 4 + 4 + 6 * 32 + 3 * 8 + 8)?; // identity length/version, Source identities and dimensions.
    }
    Ok(n)
}

// Mechanical recursive Native admission of the exact raw Source candidates.
// This supplies no initializer/graph policy; the opaque execution owns that.
fn admit_seed_candidate(
    raw: &LanguageParserJointRuntimeRawHypothesis,
) -> Result<LanguageParserJointHypothesis, SessionTransactionRefusal> {
    admit_raw_candidate(raw.hypothesis())
}
fn admit_raw_candidate(
    raw: &LanguageParserRawJointHypothesis,
) -> Result<LanguageParserJointHypothesis, SessionTransactionRefusal> {
    let p = raw.parser();
    let s = p.state();
    let state = LanguageParserState::new(
        s.basis().clone(),
        *s.committed(),
        *s.depth(),
        *s.heads(),
        s.relation0().clone(),
        s.relation1().clone(),
        s.relation2().clone(),
        s.relation3().clone(),
        *s.stack(),
        *s.token_count(),
        *s.unread(),
    )
    .map_err(SessionTransactionRefusal::Native)?;
    let parser = LanguageParserHypothesis::new(*p.active(), *p.identity(), *p.score(), state)
        .map_err(SessionTransactionRefusal::Native)?;
    LanguageParserJointHypothesis::new(*raw.choices(), parser)
        .map_err(SessionTransactionRefusal::Native)
}
