//! Whole public projection goldens, including the checked collection facts.
//! The thirty-case digest retains complete ordered widget and Face bytes.
use conduit_birth_plot::{
    BirthDraft, BirthDraftRefusal, BirthFaceBasis, BirthPlotChoice, BirthWidgetFace,
};
use conduit_body::ResidentPlot;
use conduit_core::PlotIdentity;
use sha2::{Digest, Sha256};
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
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
    }
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn measured<T>(run: impl FnOnce() -> T) -> (T, usize) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            TRACK.with(|c| c.set(false));
        }
    }
    ALLOCATIONS.with(|c| c.set(0));
    TRACK.with(|c| assert!(!c.replace(true)));
    let stop = Stop;
    let result = run();
    drop(stop);
    (result, ALLOCATIONS.with(Cell::get))
}
fn basis() -> BirthFaceBasis {
    BirthFaceBasis {
        host_id: "host/projection-golden".into(),
        boot_id: "boot/projection-golden".into(),
        encounter_id: "projection-golden".into(),
        producer_plan_id: "plan/projection-golden".into(),
        producer_plot: PlotIdentity {
            source_document_id: "source/birth".into(),
            checked_plot_id: "checked/birth".into(),
            expanded_plot_id: "expanded/birth".into(),
        },
    }
}
fn draft(state: usize) -> BirthDraft {
    let choices = (0..16)
        .map(|i| BirthPlotChoice {
            title: format!("Écho 界 {i:02}"),
            // Maximum admitted search bytes, including non-ASCII case mapping.
            search_text: format!("ΣΊΓΜΑ 音 audio {}", "x".repeat(2027)),
            plot: ResidentPlot::new(
                format!("source/{i:02}").into(),
                format!("checked/{i:02}").into(),
            ),
            refusal: (i == 15).then(|| "No admitted 音 output".into()),
            selected: match state {
                0 | 3 | 4 => false,
                1 => i == 0,
                _ => i < 15,
            },
        })
        .collect();
    let mut d = BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    if state == 3 {
        d.edit_name(d.revision(), "  ".into()).unwrap();
    }
    if state == 4 {
        d = d.allow_idle_body();
    }
    d
}
#[test]
fn whole_widget_and_portable_face_keep_canonical_projection_bytes() {
    // Initialize the shared naming catalog outside measured projection work.
    drop(draft(0));
    let basis = basis();
    let mut digest = Sha256::new();
    let mut cases = 0;
    for state in 0..5 {
        for query in [
            "",
            " \u{2003}\u{3000} ",
            "ΣΊΓΜΑ",
            "音 AUDIO",
            "ÉCHO 03",
            "absent-needle",
        ] {
            let mut d = draft(state);
            d.search_plots(d.revision(), query).unwrap();
            assert_eq!(d.choices().len(), 16);
            assert!(
                d.choices()
                    .iter()
                    .all(|choice| choice.search_text.len() == 2048)
            );
            match state {
                0 => assert_eq!(
                    d.selection(d.revision()),
                    Err(BirthDraftRefusal::EmptySelection)
                ),
                3 => assert_eq!(
                    d.selection(d.revision()),
                    Err(BirthDraftRefusal::InvalidName)
                ),
                _ => assert!(d.selection(d.revision()).is_ok()),
            }
            let ((widget, face), allocations) =
                measured(|| (d.widget_face().unwrap(), d.face(&basis).unwrap()));
            let widget_bytes = serde_json::to_vec(&widget).unwrap();
            let face_bytes = serde_json::to_vec(&face).unwrap();
            // Length framing and case labels bind complete ordered serialized values.
            for bytes in [
                state.to_string().into_bytes(),
                query.as_bytes().to_vec(),
                widget_bytes,
                face_bytes,
            ] {
                digest.update((bytes.len() as u64).to_le_bytes());
                digest.update(bytes);
            }
            println!(
                "PROJECTION_ALLOCATIONS state={state} query={query:?} allocations={allocations}"
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 30);
    let actual = format!("{:x}", digest.finalize());
    assert_eq!(
        actual,
        "e5f0a441638fc2e134136c425794ef3a0b413901f6b8519b2473d3aee7aab2b7"
    );
}
