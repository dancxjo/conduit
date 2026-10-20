//! Publication-scoped pronunciation: Language owns current commitment authority.
//! A supplied-selection profile alone cannot construct this receipt.
use crate::ipa_inventory::PreparedIpaInventory;
use crate::record_pronunciation::{
    PreparedRecordPronunciationProfile, RecordPronunciationRefusal, SuppliedRecordPronunciation,
};
use conduit_language::{
    analysis_session::CurrentCommittedDependency,
    pronunciation_selection::PreparedPronunciationSelection,
};

#[derive(Debug)]
pub enum CommittedRecordPronunciationRefusal {
    Source,
    Token,
    LexicalProfile,
    Analysis,
    Arc,
    Profile(RecordPronunciationRefusal),
}

/// Keeps the actual session authority borrowed throughout rich publication.
/// The owning session therefore cannot be revised while this value is live.
pub struct CommittedRecordPronunciation<'selection, 'inventory, 'authority, 'session> {
    supplied: SuppliedRecordPronunciation<'selection, 'inventory>,
    authority: &'authority CurrentCommittedDependency<'authority, 'session>,
}
impl<'selection, 'inventory, 'authority, 'session>
    CommittedRecordPronunciation<'selection, 'inventory, 'authority, 'session>
{
    pub fn prepare(
        profile: &PreparedRecordPronunciationProfile,
        selection: &'selection PreparedPronunciationSelection,
        inventory: &'selection PreparedIpaInventory<'inventory>,
        authority: &'authority CurrentCommittedDependency<'authority, 'session>,
    ) -> Result<Self, CommittedRecordPronunciationRefusal> {
        use CommittedRecordPronunciationRefusal::*;
        let selected = selection.request();
        let dependency = authority.receipt();
        let query = dependency.admission().query();
        // Compare complete owner values, not just text/revision hashes. The
        // opaque authority already binds replayed commitment and live frontier.
        if selected.source() != query.source() {
            return Err(Source);
        }
        let lexical = dependency.lexical();
        if selection.lexical_profile() != lexical.profile() {
            return Err(LexicalProfile);
        }
        if selected.source() != lexical.source() {
            return Err(Source);
        }
        if !lexical
            .tokens()
            .iter()
            .any(|token| token == selected.token())
        {
            return Err(Token);
        }
        if selected.analysis() != dependency.arc().dependent().revision() {
            return Err(Analysis);
        }
        if selected.basis() != dependency.arc() || selected.basis() != query.arc() {
            return Err(Arc);
        }
        let supplied = profile
            .project_supplied_selection(selection, inventory)
            .map_err(Profile)?;
        Ok(Self {
            supplied,
            authority,
        })
    }
    pub fn supplied(&self) -> &SuppliedRecordPronunciation<'selection, 'inventory> {
        &self.supplied
    }
    pub fn authority(&self) -> &'authority CurrentCommittedDependency<'authority, 'session> {
        self.authority
    }
}
