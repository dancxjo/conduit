//! The actual consumer's serde boundary: canonical static literals need no ownership.
use patchbay_graph::FaceControlKind;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
fn allocated() {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn measured<T>(run: impl FnOnce() -> T) -> (T, usize) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            TRACK.with(|track| track.set(false));
        }
    }
    ALLOCATIONS.with(|count| count.set(0));
    TRACK.with(|track| assert!(!track.replace(true)));
    let stop = Stop;
    let value = run();
    drop(stop);
    (value, ALLOCATIONS.with(Cell::get))
}

#[test]
fn canonical_static_literals_decode_without_owned_allocations() {
    let allocations = [
        r#"{"BooleanChoice":{"choices":["false","true"]}}"#,
        r#"{"ScalarNumber":{"minimum":-1,"maximum":1,"unit":"µ"}}"#,
    ]
    .map(|input| {
        let (decoded, allocations) = measured(|| serde_json::from_str::<FaceControlKind>(input));
        assert!(decoded.is_ok(), "{input}: {decoded:?}");
        allocations
    });
    assert_eq!(allocations, [0, 0]);
}

#[test]
fn escaped_equivalent_literals_and_all_owned_variants_keep_the_public_wire() {
    assert_eq!(
        serde_json::from_str::<FaceControlKind>(
            r#"{"BooleanChoice":{"choices":["f\u0061lse","tr\u0075e"]}}"#
        )
        .unwrap(),
        FaceControlKind::BooleanChoice {
            choices: ["false", "true"]
        }
    );
    assert_eq!(
        serde_json::from_str::<FaceControlKind>(
            r#"{"ScalarNumber":{"minimum":-1,"maximum":1,"unit":"\u00b5"}}"#
        )
        .unwrap(),
        FaceControlKind::ScalarNumber {
            minimum: -1,
            maximum: 1,
            unit: "µ"
        }
    );
    for wire in [
        r#"{"BooleanChoice":{"choices":["false","true"]}}"#,
        r#"{"TextChoice":{"choices":["界","other"]}}"#,
        r#"{"Number":{"minimum":0,"maximum":18446744073709551615,"unit":"ms"}}"#,
        r#"{"Number":{"minimum":0,"maximum":1,"unit":null}}"#,
        r#"{"ScalarNumber":{"minimum":-9223372036854775808,"maximum":9223372036854775807,"unit":"µ"}}"#,
        r#"{"Range":{"minimum":1,"maximum":2,"unit":"Hz"}}"#,
        r#"{"Range":{"minimum":1,"maximum":2,"unit":null}}"#,
        r#""StructuredSource""#,
        r#"{"PhysicalSource":{"maximum_bytes":4294967295}}"#,
        r#"{"ShortText":{"maximum_bytes":32}}"#,
    ] {
        let owned_input = wire.to_owned();
        let decoded: FaceControlKind = serde_json::from_str(&owned_input).unwrap();
        drop(owned_input);
        assert_eq!(serde_json::to_string(&decoded).unwrap(), wire);
    }
}

#[test]
fn noncanonical_literals_wrong_shapes_and_unknown_fields_still_refuse() {
    for wire in [
        r#"{"BooleanChoice":{"choices":[]}}"#,
        r#"{"BooleanChoice":{"choices":["false"]}}"#,
        r#"{"BooleanChoice":{"choices":["false","true","false"]}}"#,
        r#"{"BooleanChoice":{"choices":["true","false"]}}"#,
        r#"{"BooleanChoice":{"choices":["False","true"]}}"#,
        r#"{"BooleanChoice":{"choices":["false","true "]}}"#,
        r#"{"BooleanChoice":{"choices":[false,true]}}"#,
        r#"{"BooleanChoice":{"choices":null}}"#,
        r#"{"BooleanChoice":{"choices":["false","true"],"extra":0}}"#,
        r#"{"BooleanChoice":{"choices":["false","true"],"choices":["false","true"]}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":"μ"}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":"µs"}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":""}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":0}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":null}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":"µ","extra":0}}"#,
        r#"{"ScalarNumber":{"minimum":0,"maximum":1,"unit":"µ","unit":"µ"}}"#,
    ] {
        assert!(
            serde_json::from_str::<FaceControlKind>(wire).is_err(),
            "{wire}"
        );
    }
}
