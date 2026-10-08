//! Source-admitted concrete aggregate delta operations; not a parser policy.
use super::*;
impl SessionTransactionStage<'_> {
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
        admission: LanguageParserStableDependencyAdmission,
    ) -> Result<(), SessionTransactionRefusal> {
        self.fact_capacity()?;
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
        self.delta.facts.push((admission, receipt));
        Ok(())
    }
    pub(super) fn acquire_inner(
        &mut self,
        original: LanguageParserIndependentProtectedAdmission,
    ) -> Result<(), SessionTransactionRefusal> {
        if usize::from(self.old.origins) + self.delta.origins.len()
            >= usize::from(self.delta.ceiling.origins)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
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
        self.delta.insertions.push(receipt);
        self.delta.protection = next;
        Ok(())
    }
    pub(super) fn rebase_inner(
        &mut self,
        receipt: LanguageParserProtectedRebaseReceipt,
    ) -> Result<(), SessionTransactionRefusal> {
        if u64::from(self.old.rebases) + self.delta.rebases.len() as u64
            >= u64::from(self.delta.ceiling.rebases)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
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
        self.delta.rebases.push(receipt);
        Ok(())
    }
    pub(super) fn commit_inner(
        &mut self,
        query: LanguageParserJointCommitQuery,
        next: LanguageParserJointBeam,
    ) -> Result<(), SessionTransactionRefusal> {
        if u64::from(self.old.commits) + self.delta.commits.len() as u64
            >= u64::from(self.delta.ceiling.commits)
        {
            return Err(SessionTransactionRefusal::Resource(Refusal::Pressure));
        }
        if self.delta.beam.as_ref() != Some(query.fact().query().beam()) {
            return Err(SessionTransactionRefusal::Previous);
        }
        let prepared =
            PreparedRetainedCommit::prepare(query).map_err(SessionTransactionRefusal::Commit)?;
        let receipt = prepared
            .admit_output(next.clone())
            .map_err(SessionTransactionRefusal::Commit)?;
        self.delta.commits.push((prepared, receipt));
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
        let mut candidate = add(
            size(&self.delta.protection)?,
            self.delta.beam.as_ref().map(size).transpose()?.unwrap_or(0),
        )?;
        for (admission, receipt) in &self.delta.facts {
            candidate = add(candidate, add(size(admission)?, size(receipt)?)?)?;
        }
        for origin in &self.delta.origins {
            candidate = add(candidate, origin_size(origin)?)?;
        }
        for receipt in &self.delta.insertions {
            candidate = add(candidate, size(receipt)?)?;
        }
        for receipt in &self.delta.rebases {
            candidate = add(candidate, size(receipt)?)?;
        }
        for (prepared, receipt) in &self.delta.commits {
            candidate = add(candidate, commit_size(prepared, receipt)?)?;
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
