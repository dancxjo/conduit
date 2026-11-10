//! Scoring seam for the single complete-round Source traversal.
use super::*;
pub(super) struct RankRequest<'a> {
    pub slot: usize,
    pub choice: u64,
    pub branch: &'a LanguageParserScorerHypothesis,
    pub lexical: &'a LanguageParserAvailableLexical,
    pub mask: &'a LanguageParserScorerRanking,
}
pub(super) trait RoundScores {
    fn rank(
        &mut self,
        replay: &mut Replay<'_>,
        request: RankRequest<'_>,
    ) -> Result<LanguageParserScorerRanking, FallbackRoundRefusal>;
}
pub(super) struct FallbackScores;
impl RoundScores for FallbackScores {
    fn rank(
        &mut self,
        replay: &mut Replay<'_>,
        request: RankRequest<'_>,
    ) -> Result<LanguageParserScorerRanking, FallbackRoundRefusal> {
        // No model input or output can enter the deliberately reduced profile.
        let _ = (
            request.slot,
            request.choice,
            request.branch,
            request.lexical,
        );
        replay.run("language-parser-scorer-fallback", request.mask)
    }
}
