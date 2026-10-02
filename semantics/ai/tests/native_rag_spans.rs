use conduit_ai::{AnswerSpan, SourceSpan, SourceSpanUnit};
use conduit_form::rust_binding::NativeRustBinding;

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn rag_spans_are_native_nonempty_intervals() {
    round_trip(SourceSpan::new(SourceSpanUnit::Bytes, 0, u64::MAX).unwrap());
    round_trip(SourceSpan::new(SourceSpanUnit::Items, 7, 8).unwrap());
    round_trip(AnswerSpan::new(0, u32::MAX).unwrap());

    assert!(SourceSpan::new(SourceSpanUnit::Bytes, 0, 0).is_err());
    assert!(SourceSpan::new(SourceSpanUnit::Items, 9, 8).is_err());
    assert!(AnswerSpan::new(1, 1).is_err());
    assert!(AnswerSpan::new(2, 1).is_err());

    let rust = include_str!("../src/rag_semantics.rs");
    assert!(!rust.contains(concat!("pub struct ", "SourceSpan")));
    assert!(!rust.contains(concat!("pub struct ", "AnswerSpan")));
}
