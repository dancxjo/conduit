use conduitos_creche_core::{data, memory, state::State, wire::*};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[path = "../../../creche-service/allocator.rs"]
mod allocator;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct Routed;
#[global_allocator]
static HEAP: Routed = Routed;
fn guest(p: *mut u8) -> bool {
    (memory::HEAP_START..memory::HEAP_START + memory::HEAP_BYTES).contains(&(p as usize))
}
fn rounded(layout: Layout) -> usize {
    layout.size().max(1).div_ceil(32) * 32
}
// SAFETY: this single-threaded proof maps the exact exclusive production grant
// before activating guest allocation; all caller custody stays on System.
unsafe impl GlobalAlloc for Routed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !ACTIVE.load(Ordering::Relaxed) {
            return unsafe { System.alloc(layout) };
        }
        let p = unsafe { allocator::PrivateHeap.alloc(layout) };
        if !p.is_null() {
            let live = LIVE.fetch_add(rounded(layout), Ordering::Relaxed) + rounded(layout);
            PEAK.fetch_max(live, Ordering::Relaxed);
        } else {
            ACTIVE.store(false, Ordering::Relaxed);
            eprintln!(
                "PRIVATE_OOM request={} live={} peak={} capacity={}",
                layout.size(),
                LIVE.load(Ordering::Relaxed),
                PEAK.load(Ordering::Relaxed),
                memory::HEAP_BYTES
            );
            ACTIVE.store(true, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        if guest(p) {
            LIVE.fetch_sub(rounded(layout), Ordering::Relaxed);
            unsafe { allocator::PrivateHeap.dealloc(p, layout) };
        } else {
            unsafe { System.dealloc(p, layout) };
        }
    }
}
unsafe extern "C" {
    fn mmap(
        address: *mut u8,
        bytes: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: isize,
    ) -> *mut u8;
}
fn entry(state: &mut State, request: Request<'_>, transfer: &mut [u8]) -> Response {
    ACTIVE.store(true, Ordering::SeqCst);
    let result = state.execute(request, transfer);
    ACTIVE.store(false, Ordering::SeqCst);
    result.unwrap()
}
fn input(state: &mut State, transfer: &mut [u8], kind: InputKind, bytes: &[u8]) -> Response {
    entry(
        state,
        Request::Begin {
            kind,
            total: bytes.len() as u32,
        },
        transfer,
    );
    for (index, chunk) in bytes.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        entry(
            state,
            Request::Write {
                offset: (index * transfer.len()) as u32,
                window_offset: 0,
                bytes: chunk.len() as u32,
            },
            transfer,
        );
    }
    entry(state, Request::Commit, transfer)
}
pub(super) fn run() {
    // MAP_FIXED_NOREPLACE refuses an occupied range; no host mapping is replaced.
    let mapped = unsafe {
        mmap(
            memory::METADATA as *mut u8,
            memory::METADATA_BYTES,
            3,
            0x100022,
            -1,
            0,
        )
    };
    assert_eq!(
        mapped as usize,
        memory::METADATA,
        "exact private grant unavailable"
    );
    for count in [5, 16] {
        assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        PEAK.store(0, Ordering::Relaxed);
        let choices = (0..count)
            .map(|i| conduit_birth_plot::BirthPlotChoice {
                title: format!("界{} {i}", "é".repeat(20)),
                search_text: "界".repeat(500),
                plot: conduit_body::ResidentPlot::new(
                    format!("source/{i}/{}", "界".repeat(60)).into(),
                    format!("checked/{i}/{}", "界".repeat(60)).into(),
                ),
                refusal: None,
                selected: i % 2 == 0,
            })
            .collect();
        let initialization = data::encode(&data::Initialize {
            uuid: "550e8400-e29b-41d4-a716-446655440000".into(),
            choices,
            allow_idle_body: false,
        })
        .unwrap();
        let mut state = State::default();
        let mut transfer = vec![0; TRANSFER_BYTES as usize];
        assert_eq!(
            input(
                &mut state,
                &mut transfer,
                InputKind::Initialize,
                &initialization
            ),
            Response::Ready { revision: 1 }
        );
        let basis = conduit_birth_plot::BirthFaceBasis {
            host_id: "host/native".into(),
            boot_id: "boot/native".into(),
            encounter_id: "collection/current".into(),
            producer_plot: conduit_core::PlotIdentity {
                source_document_id: "source/producer".into(),
                checked_plot_id: "checked/producer".into(),
                expanded_plot_id: "expanded/producer".into(),
            },
            producer_plan_id: "plan/producer".into(),
        };
        let request = data::encode(&data::FaceRequest { revision: 1, basis }).unwrap();
        let Response::Blob {
            kind: OutputKind::Face,
            bytes,
            ..
        } = input(&mut state, &mut transfer, InputKind::FaceBasis, &request)
        else {
            panic!("Face absent")
        };
        let mut output = Vec::new();
        let mut offset = 0;
        while offset < bytes {
            let Response::Chunk { bytes: n, .. } = entry(
                &mut state,
                Request::Read {
                    offset,
                    maximum: TRANSFER_BYTES,
                },
                &mut transfer,
            ) else {
                panic!("chunk absent")
            };
            output.extend_from_slice(&transfer[..n as usize]);
            offset += n;
        }
        let face: conduit_face::Face = data::decode(&output).unwrap();
        face.validate().unwrap();
        assert_eq!(
            face.subjects
                .iter()
                .filter(|s| s.role == conduit_face::FaceRole::Plot)
                .count(),
            count
        );
        ACTIVE.store(true, Ordering::SeqCst);
        drop(state);
        ACTIVE.store(false, Ordering::SeqCst);
        assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        println!(
            "COLLECTION_PRIVATE_HEAP items={count} face_bytes={bytes} rounded_peak={} grant={}",
            PEAK.load(Ordering::Relaxed),
            memory::HEAP_BYTES
        );
    }
}
