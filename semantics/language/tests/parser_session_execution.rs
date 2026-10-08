//! Fixed Source execution-custody contract. The executor here is an explicit
//! pure fixture; ordinary target Plan execution is a separate production gate.
extern crate alloc;
pub use conduit_language::parser_session_runtime;
#[path = "../src/parser_session_execution.rs"]
pub(crate) mod execution;
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::{
    rust_binding::{BoundedSequence, NativeRustBinding},
    PortableExpressionProgram,
};
use execution::*;
use parser_session_runtime::*;
fn lexical() -> LanguageParserJointLexical {
    lexical_with_pos(&[LanguageLexicalPos::Noun, LanguageLexicalPos::Verb])
}
fn lexical_with_pos(pos: &[LanguageLexicalPos]) -> LanguageParserJointLexical {
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "fixture/joint".into(),
        "profile/1".into(),
    )
    .unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            language.clone(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "record".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        None,
    )
    .unwrap();
    let entries = BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter(pos.iter().cloned().map(|pos| {
            LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
        }))
        .unwrap(),
        "record".into(),
    )
    .unwrap()])
    .unwrap();
    let profile =
        LanguageLexicalProfile::new(entries, "fixture/joint".into(), language, provenance).unwrap();
    let tape = prepare_lexical_tape(&source, &profile, None).unwrap();
    LanguageParserJointLexical::new(tape.tape().clone(), 1).unwrap()
}

fn request() -> LanguageParserSessionSeedRequest {
    let lexical = lexical();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        lexical.tape().source().material().revision().clone(),
        lexical.tape().source().material().identity().clone(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis, relation, *lexical.token_count()).unwrap();
    LanguageParserSessionSeedRequest::new(begin, 17, lexical).unwrap()
}
pub(crate) struct Executor {
    program: PortableExpressionProgram,
    foreign: bool,
    panic_on_call: bool,
}
impl ParserSourceExecutor for Executor {
    type Error = ();
    fn entry(&self) -> &str {
        "language-parser-session-seed"
    }
    fn input_type(&self) -> &StructuredInfoType {
        &self.program.input_type
    }
    fn output_type(&self) -> &StructuredInfoType {
        &self.program.output_type
    }
    fn transact(&mut self, _: u64, input: &StructuredInfoValue) -> Result<StructuredInfoValue, ()> {
        assert!(!self.panic_on_call, "fixture target consumed then panicked");
        let mut input = LanguageParserSessionSeedRequest::from_structured(input.clone()).unwrap();
        if self.foreign {
            input = LanguageParserSessionSeedRequest::new(
                input.begin().clone(),
                *input.identity() + 1,
                input.lexical().clone(),
            )
            .unwrap();
        }
        StructuredInfoValue::from_canonical_bytes(
            &self.program.evaluate(&input.encode().unwrap()).unwrap(),
        )
        .map_err(|_| ())
    }
}
pub(crate) fn port(
    foreign: bool,
) -> PreparedParserSessionPort<
    LanguageParserSessionSeedRequest,
    LanguageParserSessionSeedProposal,
    Executor,
> {
    port_with_mode(foreign, false)
}
pub(crate) fn panic_port() -> PreparedParserSessionPort<
    LanguageParserSessionSeedRequest,
    LanguageParserSessionSeedProposal,
    Executor,
> {
    port_with_mode(false, true)
}
fn port_with_mode(
    foreign: bool,
    panic_on_call: bool,
) -> PreparedParserSessionPort<
    LanguageParserSessionSeedRequest,
    LanguageParserSessionSeedProposal,
    Executor,
> {
    let program = PortableExpressionProgram::from_canonical_hex(
        include_str!(concat!(env!("OUT_DIR"), "/parser_session_seed.hex")).trim(),
    )
    .unwrap();
    PreparedParserSessionPort::prepare(
        ParserSessionEntry::Seed,
        Executor {
            program,
            foreign,
            panic_on_call,
        },
        ParserSourceFlowLimits {
            max_invocations: 2,
            max_input_bytes: 100_000_000,
            max_output_bytes: 100_000_000,
        },
        ParserSessionVerificationLimits {
            decoded_program_bytes: 128 * 1024 * 1024,
            preparation_peak_bytes: 512 * 1024 * 1024,
            retained_bytes: 256 * 1024 * 1024,
        },
    )
    .unwrap()
}
#[test]
fn exact_seed_execution_retains_whole_original_input_output_and_ordinal() {
    let mut port = port(false);
    let input = request();
    let run = port.execute(input.clone()).unwrap();
    assert_eq!(run.entry(), ParserSessionEntry::Seed);
    assert_eq!(run.ordinal(), 0);
    assert_eq!(run.input(), &input);
    assert_eq!(run.input_bytes(), input.encode().unwrap());
    assert_eq!(run.output_bytes(), run.output().clone().encode().unwrap());
    assert!(*run.output().candidate0().hypothesis().parser().active());
    assert!(!*run.output().candidate1().hypothesis().parser().active());
    assert_eq!(
        *run.output()
            .candidate0()
            .hypothesis()
            .parser()
            .state()
            .unread(),
        0
    );
    assert_eq!(
        *run.output()
            .candidate0()
            .hypothesis()
            .parser()
            .state()
            .committed(),
        0
    );
    assert_eq!(port.next_ordinal(), 1);
    assert!(!port.is_cancelled());
}
#[test]
fn native_valid_foreign_seed_output_closes_the_consumed_port() {
    let mut port = port(true);
    assert!(matches!(
        port.execute(request()),
        Err(ParserSessionExecutionRefusal::DifferentOutput)
    ));
    assert!(port.is_cancelled());
    assert_eq!(port.next_ordinal(), 1);
    assert!(matches!(
        port.execute(request()),
        Err(ParserSessionExecutionRefusal::Flow(
            ParserSourceFlowRefusal::Cancelled
        ))
    ));
}
