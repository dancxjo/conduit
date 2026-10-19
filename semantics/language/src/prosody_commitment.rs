//! Exact checked discourse/commitment correlation, not installed currentness.
use crate::{dependency_commitment::*, parser_commitment::PreparedParserCommitment, prosody::*, *};
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};

#[derive(Debug)]
pub enum CommittedProsodyRefusal {
    Dependency(CommittedDependencyRefusal),
    Prosody(ProsodyRefusal),
    Native(NativeBindingRefusal),
    Program,
}
/// Owns the replay evidence. Callers cannot publish a supplied accepted flag.
pub struct PreparedCommittedProsody {
    dependency: PreparedCommittedDependency,
    admission: LanguageCommittedProsodyAdmission,
    intent: LanguageProsodySymbolicIntent,
}
impl PreparedCommittedProsody {
    pub fn prepare(
        lexical: &lexical::PreparedLexicalTape,
        ordinal: usize,
        discourse: &LanguageVocativeDiscourseFact,
        profile: &LanguageProsodySymbolicProfile,
        request: &LanguageParserBeamCommitRequest,
        expected: &LanguageParserBeamCommitResult,
    ) -> Result<Self, CommittedProsodyRefusal> {
        use CommittedProsodyRefusal::*;
        let dependency =
            PreparedCommittedDependency::prepare(lexical, discourse.basis(), request, expected)
                .map_err(Dependency)?;
        let rich =
            prepare_rich_prosody(lexical, ordinal, discourse, profile.base()).map_err(Prosody)?;
        let admission = LanguageCommittedProsodyAdmission::new(
            dependency.admission().clone(),
            rich.requested().clone(),
            profile.clone(),
        )
        .map_err(Native)?;
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/committed_prosody_program.hex"
        )))
        .map_err(|_| Program)?;
        let encoded = program
            .evaluate(&admission.clone().encode().map_err(Native)?)
            .map_err(|_| Program)?;
        let intent = LanguageProsodySymbolicIntent::decode(&encoded).map_err(Native)?;
        Ok(Self {
            dependency,
            admission,
            intent,
        })
    }
    pub fn admission(&self) -> &LanguageCommittedProsodyAdmission {
        &self.admission
    }
    pub fn commitment(&self) -> &PreparedParserCommitment {
        self.dependency.commitment()
    }
    pub fn dependency(&self) -> &PreparedCommittedDependency {
        &self.dependency
    }
    pub fn intent(&self) -> &LanguageProsodySymbolicIntent {
        &self.intent
    }
}

pub fn committed_prosody_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)>
{
    alloc::vec![
        (
            "LanguageDependencyExecutionContext",
            LanguageDependencyExecutionContext::semantic_type().expect("checked execution context")
        ),
        (
            "LanguageDependencyExecutionObservation",
            LanguageDependencyExecutionObservation::semantic_type()
                .expect("raw execution observation")
        ),
        (
            "LanguageDependencyExecutionResult",
            LanguageDependencyExecutionResult::semantic_type().expect("checked execution result")
        ),
        (
            "LanguageProsodyDurationIntent",
            LanguageProsodyDurationIntent::semantic_type().expect("checked prosody Type")
        ),
        (
            "LanguageProsodyRateIntent",
            LanguageProsodyRateIntent::semantic_type().expect("checked prosody Type")
        ),
        (
            "LanguageProsodyFocusIntent",
            LanguageProsodyFocusIntent::semantic_type().expect("checked prosody Type")
        ),
        (
            "LanguageProsodySymbolicProfile",
            LanguageProsodySymbolicProfile::semantic_type().expect("checked prosody Type")
        ),
        (
            "LanguageProsodySymbolicIntent",
            LanguageProsodySymbolicIntent::semantic_type().expect("checked prosody Type")
        ),
        (
            "LanguageCommittedProsodyAdmission",
            LanguageCommittedProsodyAdmission::semantic_type().expect("checked prosody Type")
        ),
        (
            "LanguageCommittedDependencyQuery",
            LanguageCommittedDependencyQuery::semantic_type().expect("checked dependency query")
        ),
        (
            "LanguageCommittedDependencyAdmission",
            LanguageCommittedDependencyAdmission::semantic_type().expect("checked dependency Type")
        ),
        (
            "LanguageProsodyFrontierResult",
            LanguageProsodyFrontierResult::semantic_type().expect("checked frontier Type")
        ),
        (
            "LanguageProsodyFrontierRequest",
            LanguageProsodyFrontierRequest::semantic_type().expect("checked prosody Type")
        ),
    ]
}
