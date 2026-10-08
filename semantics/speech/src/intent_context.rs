//! Exact context paired with the existing original utterance intent.
use crate::semantic::*;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum ContextRefusal {
    Native(NativeBindingRefusal),
    Text(crate::text_admission::TextReferenceRefusal),
    MaterialMismatch,
}
pub struct PreparedUtteranceIntentContext<'a> {
    intent: &'a SpeechUtteranceIntent,
    context: &'a SpeechUtteranceIntentContext,
    text: Option<&'a LanguageText>,
    basis: SpeechUtteranceIntentContextBasis,
    text_match: Option<LanguageTextReferenceMatch>,
}
impl<'a> PreparedUtteranceIntentContext<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn context(&self) -> &'a SpeechUtteranceIntentContext {
        self.context
    }
    pub fn text(&self) -> Option<&'a LanguageText> {
        self.text
    }
    pub fn basis(&self) -> &SpeechUtteranceIntentContextBasis {
        &self.basis
    }
    pub fn text_match(&self) -> Option<&LanguageTextReferenceMatch> {
        self.text_match.as_ref()
    }
    pub fn prepare(
        intent: &'a SpeechUtteranceIntent,
        context: &'a SpeechUtteranceIntentContext,
        text: Option<&'a LanguageText>,
    ) -> Result<Self, ContextRefusal> {
        use ContextRefusal::*;
        let basis = SpeechUtteranceIntentContextBasis::new(context.clone(), intent.clone())
            .map_err(Native)?;
        let text_match = match (context.intended_text(), text) {
            (None, None) => None,
            (Some(original), Some(material)) => {
                let reference = LanguageSegmentRef::text(
                    *original.kind(),
                    original.language().clone(),
                    original.range().clone(),
                    original.revision_id().clone(),
                    original.text_id().clone(),
                )
                .map_err(Native)?;
                let resolved =
                    crate::text_admission::resolve_text(&reference, material).map_err(Text)?;
                Some(resolved.checked().clone())
            }
            _ => return Err(MaterialMismatch),
        };
        Ok(Self {
            intent,
            context,
            text,
            basis,
            text_match,
        })
    }
}
