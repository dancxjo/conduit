//! Draft child tests of streaming_preflight; no Cargo run performed.
use super::super::graph_validation::GraphValidation;
use super::*;
fn streamed(bytes: &[u8], budget: usize) -> Result<(), Refusal> {
    let mut scan = Scanner::new(super::super::MAX_BYTES);
    for _ in 0..bytes.len().div_ceil(budget).max(1) {
        let before = scan.offset();
        match scan.step(bytes, budget)? {
            true => return Ok(()),
            false => assert!(scan.offset() > before && scan.offset() - before <= budget),
        }
    }
    panic!("scanner failed to terminate within byte-derived bound")
}
#[test]
fn every_small_turn_matches_recursive_oracle() {
    let cases: &[&[u8]] = &[
        b"[2,\"CAPSEQ1\",null]",
        b"[2,\"CAPSEQ1\",{\"Graph\":{\"graph\":null}}]",
        b"[3,1,[0],{\"x\":[2,true,false]}]",
        b"[]",
        b"[0]",
        b"[1,0]",
        b"[01,0]",
        b"[1]",
        b"[0,1]",
        b"[2,1]",
        b"[1,1,2]",
        b"{\"a\":1,}",
        b"[1,1e+2]",
        b"[1,-0.01e-2]",
        b"[1,1e]",
        b"[1,01]",
        b"[1,1.]",
        b"[1,truex]",
        b"[1,\"\\uD83D\\uDE00\"]",
        b"[1,\"\\uD83D\"]",
        b"[1,\"\\uDE00\"]",
        b"[1,\"\\uD83D\\u0041\"]",
        b"[1,\"a\\\"b\\nc\"]",
        b"[1,\"\xf0\x9f\x98\x80\"]",
        b"[1,\"\xed\xa0\x80\"]",
        b"[1,\"\xc0\x80\"]",
        b"[1,\"\xf4\x90\x80\x80\"]",
    ];
    for bytes in cases {
        let expected = super::super::preflight::validate(bytes);
        for budget in 1..=bytes.len() + 1 {
            assert_eq!(
                streamed(bytes, budget),
                expected,
                "bytes={bytes:?} budget={budget}"
            );
        }
    }
}
#[test]
fn long_strings_and_final_suffix_are_consumed_in_bounded_turns() {
    let mut bytes = alloc::vec::Vec::from(b"[1,\"".as_slice());
    bytes.extend(core::iter::repeat_n(b'x', STEP_BYTES * 3 + 7));
    bytes.extend_from_slice(b"\"]");
    assert_eq!(streamed(&bytes, STEP_BYTES), Ok(()));
    bytes.push(b'x');
    assert_eq!(streamed(&bytes, STEP_BYTES), Err(Refusal::Malformed));
}
#[test]
fn depth_bound_and_graph_prefix_remain_exact() {
    for depth in [127, 128, 129] {
        let mut bytes = alloc::vec::Vec::new();
        for _ in 0..depth {
            bytes.extend_from_slice(b"[1,");
        }
        bytes.push(b'0');
        bytes.extend(core::iter::repeat_n(b']', depth));
        assert_eq!(
            streamed(&bytes, 1),
            super::super::preflight::validate(&bytes)
        );
    }
    let bytes = b"[2,\"CAPSEQ1\",null]";
    let mut scan = GraphValidation::new(bytes.len()).unwrap();
    assert_eq!(scan.step(bytes), Ok(false));
    assert_eq!(scan.step(bytes), Ok(true));
    assert_eq!(scan.cursor(), bytes.len() * 2);
    let mut scan = GraphValidation::new(bytes.len()).unwrap();
    assert_eq!(scan.step(&bytes[..bytes.len() - 1]), Err(Refusal::Bounds));
}

fn graph_streamed(bytes: &[u8]) -> Result<(), Refusal> {
    let mut scan = GraphValidation::new(bytes.len())?;
    for _ in 0..2 * bytes.len().div_ceil(STEP_BYTES) {
        if scan.step(bytes)? {
            return Ok(());
        }
    }
    panic!("graph validation exceeded derived bound")
}
#[test]
fn deterministic_byte_mutations_match_prefix_utf8_and_grammar_oracle() {
    let inputs: &[&[u8]] = &[
        b"[2,\"CAPSEQ1\",{\"a\":[2,true,\"x\\uD83D\\uDE00\"]}]",
        b"[2,\"CAPSEQ1\",[3,-0.21e+5,null,{\"x\":false}]]",
        b"[2,\"CAPSEQ1\",[1048577]]",
        b"[2,\"CAPSEQ1\",[0001048577]]",
    ];
    for original in inputs {
        for at in 0..original.len() {
            for byte in 0..=255 {
                let mut bytes = original.to_vec();
                bytes[at] = byte;
                let expected = if bytes.starts_with(super::super::PREFIX) {
                    super::super::preflight::validate(&bytes)
                } else {
                    Err(Refusal::Malformed)
                };
                assert_eq!(
                    graph_streamed(&bytes),
                    expected,
                    "at={at}, byte={byte}, bytes={bytes:?}"
                );
            }
        }
    }
}

#[test]
fn utf8_escapes_and_counts_cross_turns_with_exact_work_and_terminal_state() {
    for pad in 0..8 {
        let mut bytes = alloc::vec::Vec::from(super::super::PREFIX);
        bytes.push(b'"');
        bytes.extend(core::iter::repeat_n(b'x', STEP_BYTES - pad));
        bytes.extend_from_slice("😀é".as_bytes());
        bytes.extend_from_slice(b"\\uD83D\\uDE00\\n\\\"\\\\");
        bytes.extend(core::iter::repeat_n(b'x', STEP_BYTES + pad));
        bytes.extend_from_slice(b"\"]");
        assert_eq!(super::super::preflight::validate(&bytes), Ok(()));
        let mut scan = GraphValidation::new(bytes.len()).unwrap();
        let turns = 2 * bytes.len().div_ceil(STEP_BYTES);
        for turn in 0..turns {
            let before = scan.cursor();
            assert_eq!(scan.step(&bytes), Ok(turn + 1 == turns));
            let work = scan.cursor() - before;
            assert!(work > 0 && work <= STEP_BYTES);
        }
        assert_eq!(scan.cursor(), 2 * bytes.len());
        assert_eq!(scan.step(&bytes), Err(Refusal::Order));
    }
    let bytes = b"[2,\"CAPSEQ1\",[0001048577]]";
    assert_eq!(graph_streamed(bytes), Err(Refusal::Bounds));
    let bytes = b"[2,\"CAPSEQ1\",[0001048577],\xff]";
    assert_eq!(graph_streamed(bytes), Err(Refusal::Malformed));
    let mut scan = GraphValidation::new(bytes.len()).unwrap();
    assert_eq!(scan.step(bytes), Err(Refusal::Malformed));
    assert_eq!(scan.step(bytes), Err(Refusal::Order));
}
