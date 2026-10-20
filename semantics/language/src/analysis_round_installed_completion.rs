//! One preadmitted, one-shot Native materialization slot reusing cursor law proof.
//! This prerequisite does not detach evidence, publish an owner, or grant Current.
use super::{CursorInstanceHandle, CursorProgress, CursorRefusal, PreparedRoundCursor};
use crate::analysis_epoch_owner::beam_slot::{PreparedBeamSlot, SharedBeam};
use crate::generated::LanguageParserScorerBeamPreparedStorage;
use crate::LanguageParserScorerBeam;
use conduit_plot::{
    rust_binding::{
        PreparedDecodeBounds, PreparedDecodeGate, PreparedDecodeProgress, PreparedDecodeRefusal,
        PreparedNativeRefusal,
    },
    CheckedSyntaxDocument,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstalledCompletionProgress {
    Prepared,
    Decoding,
    Assembling,
    Filling,
    Complete,
    Failed,
}
#[derive(Debug)]
pub enum InstalledCompletionRefusal {
    Preparation,
    ForeignCursor,
    IncompleteCursor,
    Cancelled,
    Pressure,
    State,
    Native(PreparedNativeRefusal),
    Cursor(CursorRefusal),
    Decode(PreparedDecodeRefusal),
}
/// All buffers and the unique empty result slot are allocated before Play.
/// A finite controller must retain one entry per admitted round/history slot;
/// this entry cannot reset or recycle while its resulting receipt remains held.
pub struct PreparedInstalledRoundCompletion<'d> {
    decoder: Option<LanguageParserScorerBeamPreparedStorage<'d>>,
    decode_retained: Option<PreparedDecodeGate<'d>>,
    empty: Option<PreparedBeamSlot>,
    completed: Option<SharedBeam>,
    unfilled_native: Option<LanguageParserScorerBeam>,
    cursor: Option<CursorInstanceHandle>,
    progress: InstalledCompletionProgress,
    failure: Option<InstalledCompletionRefusal>,
}
impl<'d> PreparedInstalledRoundCompletion<'d> {
    pub fn prepare(
        document: &'d CheckedSyntaxDocument,
    ) -> Result<Self, InstalledCompletionRefusal> {
        if document.source_document_id.as_str() != crate::parser_commitment::COMMITMENT_SOURCE_ID {
            return Err(InstalledCompletionRefusal::Preparation);
        }
        let decoder = LanguageParserScorerBeam::prepare_storage(
            document,
            PreparedDecodeBounds {
                canonical_bytes: 65536,
                leaf_steps: LanguageParserScorerBeam::PREPARED_STORAGE_LEAF_STEPS,
                text_bytes: LanguageParserScorerBeam::PREPARED_STORAGE_TEXT_BYTES,
            },
        )
        .map_err(InstalledCompletionRefusal::Decode)?;
        Ok(Self {
            decoder: Some(decoder),
            decode_retained: None,
            empty: Some(PreparedBeamSlot::prepare_empty()),
            completed: None,
            unfilled_native: None,
            cursor: None,
            progress: InstalledCompletionProgress::Prepared,
            failure: None,
        })
    }
    pub fn progress(&self) -> InstalledCompletionProgress {
        self.progress
    }
    pub fn failure(&self) -> Option<&InstalledCompletionRefusal> {
        self.failure.as_ref()
    }
    pub fn retained_decoder_gate(&self) -> Option<&PreparedDecodeGate<'d>> {
        self.decode_retained.as_ref()
    }
    /// Already admitted Native output is inspectable even after cancellation;
    /// this getter conveys neither owner currentness nor publication authority.
    pub fn result(&self) -> Option<&LanguageParserScorerBeam> {
        self.completed
            .as_ref()
            .map(SharedBeam::get)
            .or(self.unfilled_native.as_ref())
    }
    fn correlate(
        &self,
        cursor: &PreparedRoundCursor<'_>,
    ) -> Result<(), InstalledCompletionRefusal> {
        if self
            .cursor
            .as_ref()
            .is_some_and(|binding| !binding.matches(cursor))
        {
            return Err(InstalledCompletionRefusal::ForeignCursor);
        }
        if cursor.progress() != CursorProgress::Complete {
            return Err(InstalledCompletionRefusal::IncompleteCursor);
        }
        cursor
            .completed_result_admission()
            .map_err(InstalledCompletionRefusal::Cursor)?;
        Ok(())
    }
    /// One generated leaf, or one finite binding/assembly/fill.
    /// Reuses the cursor's actual successful Beam admission; no law replay.
    /// Errors borrow retained failure evidence rather than cloning it.
    pub fn step(
        &mut self,
        cursor: &PreparedRoundCursor<'_>,
    ) -> Result<InstalledCompletionProgress, &InstalledCompletionRefusal> {
        if self.failure.is_none() {
            if let Err(refusal) = self.step_inner(cursor) {
                self.failure = Some(refusal);
                self.progress = InstalledCompletionProgress::Failed;
            }
        }
        match self.failure.as_ref() {
            Some(refusal) => Err(refusal),
            None => Ok(self.progress),
        }
    }
    fn step_inner(
        &mut self,
        cursor: &PreparedRoundCursor<'_>,
    ) -> Result<(), InstalledCompletionRefusal> {
        self.correlate(cursor)?;
        match self.progress {
            InstalledCompletionProgress::Prepared => {
                self.cursor = Some(cursor.instance_handle());
                self.progress = InstalledCompletionProgress::Decoding;
            }
            InstalledCompletionProgress::Decoding => {
                let witness = cursor
                    .completed_result_admission()
                    .map_err(InstalledCompletionRefusal::Cursor)?
                    .admitted()
                    .map_err(InstalledCompletionRefusal::Native)?;
                let decoder = self
                    .decoder
                    .as_mut()
                    .ok_or(InstalledCompletionRefusal::State)?;
                if decoder
                    .step(&witness)
                    .map_err(InstalledCompletionRefusal::Decode)?
                    == PreparedDecodeProgress::Complete
                {
                    self.progress = InstalledCompletionProgress::Assembling;
                }
            }
            InstalledCompletionProgress::Assembling => {
                let witness = cursor
                    .completed_result_admission()
                    .map_err(InstalledCompletionRefusal::Cursor)?
                    .admitted()
                    .map_err(InstalledCompletionRefusal::Native)?;
                let decoder = self
                    .decoder
                    .take()
                    .ok_or(InstalledCompletionRefusal::State)?;
                // Required narrow generated API amendment: retain, never drop,
                // the decoder gate's original canonical allocation on success.
                match decoder.finish_retained(&witness) {
                    Ok((native, retained)) => {
                        self.unfilled_native = Some(native);
                        self.decode_retained = Some(retained);
                        self.progress = InstalledCompletionProgress::Filling;
                    }
                    Err((reason, decoder)) => {
                        self.decoder = Some(decoder);
                        return Err(InstalledCompletionRefusal::Decode(reason));
                    }
                }
            }
            InstalledCompletionProgress::Filling => {
                if self.empty.is_none() || self.unfilled_native.is_none() {
                    return Err(InstalledCompletionRefusal::State);
                }
                let empty = self.empty.take().ok_or(InstalledCompletionRefusal::State)?;
                let native = self
                    .unfilled_native
                    .take()
                    .ok_or(InstalledCompletionRefusal::State)?;
                match empty.fill(native) {
                    Ok(shared) => {
                        self.completed = Some(shared);
                        self.progress = InstalledCompletionProgress::Complete;
                    }
                    Err((empty, native)) => {
                        self.empty = Some(empty);
                        self.unfilled_native = Some(native);
                        return Err(InstalledCompletionRefusal::Pressure);
                    }
                }
            }
            InstalledCompletionProgress::Complete => (),
            InstalledCompletionProgress::Failed => return Err(InstalledCompletionRefusal::State),
        }
        Ok(())
    }
    /// Only the exact completed cursor can prepare a publication receipt.
    /// Sharing this immutable slot itself grants no Current authority.
    pub(in crate::analysis_epoch_owner::fallback_round) fn completed_slot(
        &self,
        cursor: &PreparedRoundCursor<'_>,
    ) -> Result<SharedBeam, InstalledCompletionRefusal> {
        if self.failure.is_some() || self.progress != InstalledCompletionProgress::Complete {
            return Err(InstalledCompletionRefusal::State);
        }
        self.correlate(cursor)?;
        self.completed
            .clone()
            .ok_or(InstalledCompletionRefusal::State)
    }
    /// Terminal local cancellation. The external cursor, actual Source/ACK/model
    /// evidence, decoder slots and any admitted result remain owned unchanged.
    pub fn cancel(&mut self) {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.cancel();
        }
        self.failure = Some(InstalledCompletionRefusal::Cancelled);
        self.progress = InstalledCompletionProgress::Failed;
    }
}
