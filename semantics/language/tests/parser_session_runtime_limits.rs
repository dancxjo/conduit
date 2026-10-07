use conduit_core::{StructuredInfoType, StructuredInfoValue};
// Include the unwired public module without changing either frozen library.
#[allow(dead_code)]
#[path = "../src/parser_session_runtime.rs"]
mod parser_session_runtime;
use conduit_language::{LanguageTextId, LanguageTextRevisionId};
use conduit_plot::rust_binding::NativeRustBinding;
use parser_session_runtime::*;
use std::{cell::RefCell, rc::Rc};

struct Executor {
    input: StructuredInfoType,
    output: StructuredInfoType,
    calls: Rc<RefCell<Vec<u64>>>,
    fail: bool,
    foreign: bool,
}
impl ParserSourceExecutor for Executor {
    type Error = &'static str;
    fn entry(&self) -> &str {
        "fixture/port-only"
    }
    fn input_type(&self) -> &StructuredInfoType {
        &self.input
    }
    fn output_type(&self) -> &StructuredInfoType {
        &self.output
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &StructuredInfoValue,
    ) -> Result<StructuredInfoValue, Self::Error> {
        self.calls.borrow_mut().push(ordinal);
        if self.fail {
            return Err("input may already have been consumed");
        }
        if self.foreign {
            return Ok(LanguageTextId::new("same-bytes".into())
                .unwrap()
                .into_structured()
                .unwrap());
        }
        Ok(input.clone())
    }
}
fn executor() -> Executor {
    Executor {
        input: LanguageTextRevisionId::semantic_type().unwrap(),
        output: LanguageTextRevisionId::semantic_type().unwrap(),
        calls: Rc::new(RefCell::new(Vec::new())),
        fail: false,
        foreign: false,
    }
}
type Flow = PreparedParserSourceFlow<LanguageTextRevisionId, LanguageTextRevisionId, Executor>;
fn input() -> LanguageTextRevisionId {
    LanguageTextRevisionId::new("same-bytes".into()).unwrap()
}
#[test]
fn retained_executor_gets_each_ordinal_once_and_returns_admitted_native_identity() {
    let executor = executor();
    let calls = executor.calls.clone();
    let mut flow = Flow::new(executor, limits()).unwrap();
    assert_eq!(flow.transact(input()).unwrap(), input());
    assert_eq!(flow.transact(input()).unwrap(), input());
    assert_eq!(*calls.borrow(), [0, 1]);
    assert_eq!(flow.next_ordinal(), 2);
    assert!(!flow.is_poisoned());
}
#[test]
fn shape_compatible_foreign_nominal_ports_refuse_before_execution() {
    let mut executor = executor();
    let calls = executor.calls.clone();
    executor.input = LanguageTextId::semantic_type().unwrap();
    assert!(matches!(
        Flow::new(executor, limits()),
        Err(ParserSourceFlowRefusal::InputType)
    ));
    assert!(calls.borrow().is_empty());
    let mut executor = self::executor();
    executor.output = LanguageTextId::semantic_type().unwrap();
    assert!(matches!(
        Flow::new(executor, limits()),
        Err(ParserSourceFlowRefusal::OutputType)
    ));
}
#[test]
fn failed_or_foreign_response_cannot_silently_replay_a_consumed_ordinal() {
    for foreign in [false, true] {
        let mut executor = executor();
        let calls = executor.calls.clone();
        executor.foreign = foreign;
        executor.fail = !foreign;
        let mut flow = Flow::new(executor, limits()).unwrap();
        assert!(flow.transact(input()).is_err());
        assert!(flow.is_poisoned());
        assert_eq!(flow.next_ordinal(), 0);
        assert!(matches!(
            flow.transact(input()),
            Err(ParserSourceFlowRefusal::Poisoned)
        ));
        assert_eq!(*calls.borrow(), [0]);
    }
}

fn limits() -> ParserSourceFlowLimits {
    ParserSourceFlowLimits {
        max_invocations: 2,
        max_input_bytes: 4096,
        max_output_bytes: 4096,
    }
}
#[test]
fn pressure_and_cancel_refuse_before_executor_consumption() {
    let executor = executor();
    let calls = executor.calls.clone();
    let mut flow = Flow::new(executor, limits()).unwrap();
    assert_eq!(flow.limits(), limits());
    flow.transact(input()).unwrap();
    flow.transact(input()).unwrap();
    assert!(matches!(
        flow.transact(input()),
        Err(ParserSourceFlowRefusal::InvocationPressure)
    ));
    assert_eq!(flow.next_ordinal(), 2);
    assert_eq!(*calls.borrow(), [0, 1]);
    assert!(!flow.is_poisoned());
    flow.cancel();
    assert!(flow.is_cancelled());
    assert!(matches!(
        flow.transact(input()),
        Err(ParserSourceFlowRefusal::Cancelled)
    ));
    assert_eq!(*calls.borrow(), [0, 1]);
    assert_eq!(flow.next_ordinal(), 2);
    let executor = self::executor();
    let calls = executor.calls.clone();
    let mut flow = Flow::new(
        executor,
        ParserSourceFlowLimits {
            max_input_bytes: 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(matches!(
        flow.transact(input()),
        Err(ParserSourceFlowRefusal::InputPressure)
    ));
    assert!(calls.borrow().is_empty());
    assert_eq!(flow.next_ordinal(), 0);
    assert!(!flow.is_poisoned());
}
#[test]
fn response_pressure_poisoning_preserves_ordinal_and_prevents_retry() {
    let executor = executor();
    let calls = executor.calls.clone();
    let mut flow = Flow::new(
        executor,
        ParserSourceFlowLimits {
            max_output_bytes: 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(matches!(
        flow.transact(input()),
        Err(ParserSourceFlowRefusal::OutputPressure)
    ));
    assert!(flow.is_poisoned());
    assert_eq!(flow.next_ordinal(), 0);
    assert!(matches!(
        flow.transact(input()),
        Err(ParserSourceFlowRefusal::Poisoned)
    ));
    assert_eq!(*calls.borrow(), [0]);
}
#[test]
fn zero_resources_refuse_without_consumption() {
    for limits in [
        ParserSourceFlowLimits {
            max_invocations: 0,
            ..limits()
        },
        ParserSourceFlowLimits {
            max_input_bytes: 0,
            ..limits()
        },
        ParserSourceFlowLimits {
            max_output_bytes: 0,
            ..limits()
        },
    ] {
        let executor = executor();
        let calls = executor.calls.clone();
        assert!(matches!(
            Flow::new(executor, limits),
            Err(ParserSourceFlowRefusal::Limits)
        ));
        assert!(calls.borrow().is_empty());
    }
}

#[test]
fn exact_canonical_byte_boundary_and_cancel_before_first_ingress() {
    let bytes = input()
        .into_structured()
        .unwrap()
        .canonical_bytes()
        .unwrap()
        .len() as u32;
    let executor = executor();
    let calls = executor.calls.clone();
    let mut flow = Flow::new(
        executor,
        ParserSourceFlowLimits {
            max_input_bytes: bytes,
            max_output_bytes: bytes,
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(flow.entry(), "fixture/port-only");
    assert_eq!(flow.transact(input()).unwrap(), input());
    assert_eq!(*calls.borrow(), [0]);
    let executor = self::executor();
    let calls = executor.calls.clone();
    let mut flow = Flow::new(executor, limits()).unwrap();
    flow.cancel();
    flow.cancel();
    assert!(matches!(
        flow.transact(input()),
        Err(ParserSourceFlowRefusal::Cancelled)
    ));
    assert!(calls.borrow().is_empty());
    assert_eq!(flow.next_ordinal(), 0);
    assert!(!flow.is_poisoned());
}
