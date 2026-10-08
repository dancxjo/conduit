from pathlib import Path
p=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/work/production-custody/cursor-draft')
f=p/'parser_session_window8_observe.rs';s=f.read_text();s=s.replace('    pub(crate) fn available_candidates', '    pub(crate) fn snapshot(&self) -> &[u8] { &self.snapshot }\n    pub(crate) fn snapshot_capacity(&self) -> usize { self.snapshot.capacity() }\n    pub(crate) fn available_candidates',1);f.write_text(s)
f=p/'parser_session_window8_working_set.rs';s=f.read_text().replace('    history: Window8HistoryOwnership,','    history: Window8HistoryOwnership,\n    maximum_complete_bytes: usize,',1);s=s.replace('impl PreparedWindow8WorkingSet {','''impl PreparedWindow8WorkingSet {
    /// Adds an enclosing owner's fully declared inline/backing storage before
    /// that owner allocates. The original complete profile remains immutable.
    pub(crate) fn reserve_owner_delta(&mut self, preparation: usize, retained: usize) -> Result<(), WorkingSetRefusal> {
        let complete = add(self.receipt.complete_declared_bytes_bound, preparation.max(retained))?;
        let new_preparation = add(self.receipt.new_preparation_bytes_bound, preparation)?;
        let new_retained = add(self.receipt.new_retained_bytes_bound, retained)?;
        if complete > self.maximum_complete_bytes { return Err(WorkingSetRefusal); }
        self.receipt.complete_declared_bytes_bound = complete;
        self.receipt.new_preparation_bytes_bound = new_preparation;
        self.receipt.new_retained_bytes_bound = new_retained;
        Ok(())
    }
''',1);s=s.replace('            history: limits.history,','            history: limits.history,\n            maximum_complete_bytes: limits.maximum_complete_bytes,',1);f.write_text(s)
f=p/'parser_session_window8_run.rs';s=f.read_text();s=s.replace('pub(crate) struct InitialRunResult {','#[derive(Clone, Copy, Debug)]\npub(crate) struct InitialRunResult {',1);pos=s.index('#[derive(Debug)]\npub(crate) struct InitialRunRefusal');s=s[:pos]+'''/// Every locator resolves only in the Book published alongside this cursor.
/// No Native snapshot supplied by a caller can construct this cursor.
pub(crate) struct InitialRunCursor {
    pub(crate) availability: usize,
    pub(crate) initializer: usize,
    pub(crate) seed: Option<usize>,
    pub(crate) beam: Option<usize>,
    pub(crate) token_count: u64,
    pub(crate) next_identity: u64,
    pub(crate) classes: Option<[crate::parser_session_window8_classes::Window8ClassDerivation; 76]>,
    pub(crate) observations: Option<crate::parser_session_window8_observe::EpochObservations>,
    pub(crate) snapshot: alloc::vec::Vec<u8>,
    pub(crate) progress: InitialRunResult,
}
'''+s[pos:];s=s.replace('    limits: InitialRunLimits<\'_>,','    limits: InitialRunLimits<\'_>,\n    mut retained_snapshot: alloc::vec::Vec<u8>,',1);s=s.replace('        InitialRunResult,','        InitialRunCursor,',1);s=s.replace('''        if limits.maximum_epochs == 0 {''','''        if limits.maximum_epochs == 0 || !retained_snapshot.is_empty() || retained_snapshot.capacity() < banks.observations.snapshot_capacity() {''',1);s=s.replace('''            return Ok(InitialRunResult {
                stop: InitialRunStop::WaitingEmpty,
                beam: None,
                epochs: 0,
                model_calls: 0,
            });''','''            return Ok(InitialRunCursor {
                availability: initial.availability, initializer: initial.initial,
                seed: None, beam: None, token_count: initial.count, next_identity: 0,
                classes: None, observations: None, snapshot: retained_snapshot,
                progress: InitialRunResult { stop: InitialRunStop::WaitingEmpty, beam: None, epochs: 0, model_calls: 0 },
            });''',1);s=s.replace('        let mut stop = InitialRunStop::EpochLimit;','        let mut stop = InitialRunStop::EpochLimit;\n        let mut observations = None;',1);s=s.replace('''            crate::parser_session_window8_observe::observe(''','''            observations = Some(crate::parser_session_window8_observe::observe(''',1);s=s.replace('''            .map_err(|_| InitialRunRefusal)?;
            if completion.all_complete''','''            .map_err(|_| InitialRunRefusal)?);
            let snapshot = banks.observations.snapshot();
            if snapshot.len() > retained_snapshot.capacity() { return Err(InitialRunRefusal); }
            retained_snapshot.clear();
            retained_snapshot.extend_from_slice(snapshot);
            if completion.all_complete''',1);s=s.replace('''        Ok(InitialRunResult {
            stop,
            beam: Some(beam),
            epochs,
            model_calls: calls,
        })''','''        Ok(InitialRunCursor {
            availability: initial.availability, initializer: initial.initial,
            seed: Some(seed), beam: Some(beam), token_count: initial.count,
            next_identity: identity, classes: Some(classes), observations,
            snapshot: retained_snapshot,
            progress: InitialRunResult { stop, beam: Some(beam), epochs, model_calls: calls },
        })''',1);f.write_text(s)
f=p/'parser_session_window8_session.rs';s=f.read_text().replace('InitialRunLimits, InitialRunResult','InitialRunLimits, InitialRunResult, InitialRunCursor');s=s.replace('    current: Option<Rc<Window8Book>>,','    current: Option<RetainedWindow8Revision>,\n    prepared_snapshot: Option<alloc::vec::Vec<u8>>,',1);s=s.replace('pub(crate) struct PublishedWindow8Revision {','''struct RetainedWindow8Revision {
    book: Rc<Window8Book>,
    cursor: InitialRunCursor,
}
pub(crate) struct PublishedWindow8Revision {''',1);s=s.replace('''        Ok(Self {
            factory,''','''        // The prepared factory already charged its own inline owner. Charge
        // only the additional Session inline bytes plus exact snapshot backing.
        let frame = factory.working.observations.snapshot_capacity();
        let inline = core::mem::size_of::<Self>().checked_sub(core::mem::size_of::<PreparedWindow8Factory<S,N>>()).ok_or(Window8SessionRefusal)?;
        let delta = inline.checked_add(frame).ok_or(Window8SessionRefusal)?;
        if factory.working.reserve_owner_delta(delta, delta).is_err() {
            factory.registry.cancel_all(); return Err(Window8SessionRefusal);
        }
        let mut snapshot = alloc::vec::Vec::new();
        if snapshot.try_reserve_exact(frame).is_err() || snapshot.capacity() != frame {
            factory.registry.cancel_all(); return Err(Window8SessionRefusal);
        }
        Ok(Self {
            factory,
            prepared_snapshot: Some(snapshot),''',1);s=s.replace('''            let (guard, mut book, progress) =
                parser_session_window8_run::execute(stage, banks, &identity, limits)''','''            let snapshot = self.prepared_snapshot.take().ok_or(Window8SessionRefusal)?;
            let (guard, mut book, cursor) =
                parser_session_window8_run::execute(stage, banks, &identity, limits, snapshot)''',1);s=s.replace('''            self.current = Some(Rc::clone(&book));''','''            let progress = cursor.progress;
            self.current = Some(RetainedWindow8Revision { book: Rc::clone(&book), cursor });''',1);s=s.replace('        self.current.as_deref()','        self.current.as_ref().map(|current| current.book.as_ref())',1);s=s.replace('    pub(crate) fn cancel(&mut self) {','''    pub(crate) fn progress(&self) -> Option<InitialRunResult> { self.current.as_ref().map(|current| current.cursor.progress) }
    pub(crate) fn cancel(&mut self) {''',1);f.write_text(s)
