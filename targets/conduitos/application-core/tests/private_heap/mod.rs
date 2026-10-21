use conduitos_application_core::{
    data::{self, Command, Initial, Reply},
    memory,
    private_heap::{self, PrivateHeap},
    state::State,
    wire::{Request, Response},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static STAGE: AtomicUsize = AtomicUsize::new(0);
fn report(label: &str, mut number: usize) {
    let mut bytes = [0u8; 96];
    let mut end = 0;
    for byte in label.bytes() {
        bytes[end] = byte;
        end += 1;
    }
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    loop {
        at -= 1;
        digits[at] = b'0' + (number % 10) as u8;
        number /= 10;
        if number == 0 {
            break;
        }
    }
    for byte in &digits[at..] {
        bytes[end] = *byte;
        end += 1;
    }
    bytes[end] = b'\n';
    end += 1;
    unsafe {
        write(2, bytes.as_ptr(), end);
    }
}
struct Routed;
#[global_allocator]
static HEAP: Routed = Routed;
fn guest(pointer: *mut u8) -> bool {
    (memory::HEAP_START..memory::HEAP_START + memory::HEAP_BYTES).contains(&(pointer as usize))
}
fn rounded(layout: Layout) -> usize {
    layout.size().max(1).div_ceil(32) * 32
}
// SAFETY: this single-threaded executable installs the exact exclusive fixed
// grant before activating routing; all other host allocations remain on System.
unsafe impl GlobalAlloc for Routed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !ACTIVE.load(Ordering::Relaxed) {
            return unsafe { System.alloc(layout) };
        }
        let pointer = unsafe { PrivateHeap.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(rounded(layout), Ordering::Relaxed) + rounded(layout);
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        if pointer.is_null() {
            report("OOM stage=", STAGE.load(Ordering::Relaxed));
            report("OOM request=", layout.size());
            report("OOM live=", LIVE.load(Ordering::Relaxed));
            report("OOM peak=", PEAK.load(Ordering::Relaxed));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if guest(pointer) {
            LIVE.fetch_sub(rounded(layout), Ordering::Relaxed);
            unsafe { PrivateHeap.dealloc(pointer, layout) };
        } else {
            unsafe { System.dealloc(pointer, layout) };
        }
    }
}
unsafe extern "C" {
    fn write(fd: i32, bytes: *const u8, length: usize) -> isize;
    fn mmap(
        pointer: *mut u8,
        length: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: isize,
    ) -> *mut u8;
}
const FIXTURE: &[u8] = include_bytes!("../fixtures/startup.catz");
// Retained input compatibility exists only in this fixture, never the protocol.
#[derive(serde::Deserialize)]
enum LegacyFixture {
    Init { applications: [Option<Initial>; 4] },
}

fn on_caller<T>(operation: impl FnOnce() -> T) -> T {
    let active = ACTIVE.swap(false, Ordering::SeqCst);
    let value = operation();
    ACTIVE.store(active, Ordering::SeqCst);
    value
}
fn exchange(state: &mut State, command: &Command, transfer: &mut [u8]) -> Reply {
    let input = on_caller(|| data::encode_command(command).unwrap());
    state
        .execute(
            Request::Begin {
                bytes: input.len() as u32,
            },
            transfer,
        )
        .unwrap();
    for (index, chunk) in input.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        state
            .execute(
                Request::Write {
                    offset: (index * transfer.len()) as u32,
                    bytes: chunk.len() as u32,
                },
                transfer,
            )
            .unwrap();
    }
    let Response::Blob { bytes } = state.execute(Request::Commit, transfer).unwrap() else {
        panic!("missing reply");
    };
    drop(input);
    let mut output = Vec::new();
    for offset in (0..bytes as usize).step_by(transfer.len()) {
        let Response::Chunk { bytes, .. } = state
            .execute(
                Request::Read {
                    offset: offset as u32,
                    maximum: transfer.len() as u32,
                },
                transfer,
            )
            .unwrap()
        else {
            panic!("missing chunk");
        };
        on_caller(|| output.extend_from_slice(&transfer[..bytes as usize]));
    }
    on_caller(|| data::decode_reply(&output).unwrap())
}
pub fn run() {
    // MAP_FIXED_NOREPLACE never overwrites a host mapping; fail rather than
    // silently measuring an alternate address or allocator implementation.
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
    assert_eq!(mapped as usize, memory::METADATA);
    let LegacyFixture::Init { applications } = data::decode(FIXTURE).unwrap();
    let Some(Initial::Patchbay { targets, selected }) = &applications[0] else {
        panic!("Patchbay fixture");
    };
    assert_eq!(*selected, 1);
    let Some(Initial::Tutorial { evidence }) = &applications[1] else {
        panic!("Tutorial fixture");
    };
    let expected_tutorial = conduitos_application_core::tutorial::project(evidence, false)
        .unwrap()
        .encode()
        .unwrap();
    // Root's caller-side command was prepared outside this grant. Inside the
    // service this measures incoming, typed decode, controllers and outgoing.
    ACTIVE.store(true, Ordering::SeqCst);
    let mut state = State::default();
    let mut transfer = [0u8; memory::TRANSFER_BYTES];
    STAGE.store(1, Ordering::Relaxed);
    assert!(matches!(
        exchange(
            &mut state,
            &Command::PrepareInit {
                manifest: data::InitManifest::from_initial(&applications),
            },
            &mut transfer
        ),
        Reply::Prepared
    ));
    for (slot, initial) in applications.iter().enumerate() {
        match initial {
            None => {}
            Some(Initial::Patchbay { targets, .. }) => {
                for (target, value) in targets.iter().enumerate() {
                    if let Some(value) = value {
                        let component = on_caller(|| Command::PrepareTarget {
                            slot: slot as u8,
                            target: target as u8,
                            value: value.clone(),
                        });
                        assert!(matches!(
                            exchange(&mut state, &component, &mut transfer),
                            Reply::Prepared
                        ));
                    }
                }
            }
            Some(Initial::Tutorial { evidence }) => {
                let component = on_caller(|| Command::PrepareTutorial {
                    slot: slot as u8,
                    evidence: evidence.clone(),
                });
                assert!(matches!(
                    exchange(&mut state, &component, &mut transfer),
                    Reply::Prepared
                ));
            }
            Some(Initial::Tour) => {
                assert!(matches!(
                    exchange(
                        &mut state,
                        &Command::PrepareTour { slot: slot as u8 },
                        &mut transfer
                    ),
                    Reply::Prepared
                ));
            }
        }
    }
    assert!(matches!(
        exchange(&mut state, &Command::FinishInit, &mut transfer),
        Reply::Initialized
    ));
    report("after Init peak=", PEAK.load(Ordering::Relaxed));
    STAGE.store(2, Ordering::Relaxed);
    let init_peak = PEAK.load(Ordering::Relaxed);
    let Reply::Applied { snapshot, proposal } = exchange(
        &mut state,
        &Command::Apply {
            slot: 1,
            input: Vec::new(),
        },
        &mut transfer,
    ) else {
        panic!("Tutorial reply");
    };
    assert_eq!(snapshot.view, expected_tutorial);
    assert!(proposal.is_none());
    drop(snapshot);
    for target in [1u8, 2, 3, 2, 1] {
        STAGE.store(10 + usize::from(target), Ordering::Relaxed);
        assert!(matches!(
            exchange(
                &mut state,
                &Command::Select { slot: 0, target },
                &mut transfer
            ),
            Reply::Updated { .. }
        ));
        STAGE.store(20 + usize::from(target), Ordering::Relaxed);
        let Reply::Graph { graph: Some(graph) } =
            exchange(&mut state, &Command::Graph { slot: 0 }, &mut transfer)
        else {
            panic!("Graph reply");
        };
        assert_eq!(graph, targets[usize::from(target)].as_ref().unwrap().graph);
    }
    for target in [1usize, 2, 3] {
        STAGE.store(30 + target, Ordering::Relaxed);
        let projection = on_caller(|| {
            let graph = targets[target].as_ref().unwrap().graph.clone();
            let scope = conduitos_application_core::graph_facts::Scope::new(
                conduit_core::HostId::from("host/allocator-proof"),
                conduit_core::BootId::from("boot/allocator-proof"),
                evidence.body_id.clone(),
                1,
                &graph,
                false,
            )
            .unwrap();
            Command::PrepareGraphProjection { graph, scope }
        });
        assert!(matches!(
            exchange(&mut state, &projection, &mut transfer),
            Reply::Prepared
        ));
        let Reply::GraphFacts { contribution } =
            exchange(&mut state, &Command::FinishGraphProjection, &mut transfer)
        else {
            panic!("Graph projection reply");
        };
        let Command::PrepareGraphProjection { scope, .. } = projection else {
            unreachable!()
        };
        assert_eq!(contribution.scope, scope);
        assert!(!contribution.subjects.is_empty());
    }
    // A full second compressed graph must refuse at the pending command tag,
    // without allocating another typed graph beside the retained first query.
    let pending = on_caller(|| {
        let graph = targets[3].as_ref().unwrap().graph.clone();
        let scope = conduitos_application_core::graph_facts::Scope::new(
            "host/allocator-proof".into(),
            "boot/allocator-proof".into(),
            evidence.body_id.clone(),
            1,
            &graph,
            false,
        )
        .unwrap();
        Command::PrepareGraphProjection { graph, scope }
    });
    assert!(matches!(
        exchange(&mut state, &pending, &mut transfer),
        Reply::Prepared
    ));
    let duplicate = on_caller(|| data::encode_command(&pending).unwrap());
    assert!(duplicate.starts_with(b"CAPZ"));
    state
        .execute(
            Request::Begin {
                bytes: duplicate.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    for (index, chunk) in duplicate.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        state
            .execute(
                Request::Write {
                    offset: (index * transfer.len()) as u32,
                    bytes: chunk.len() as u32,
                },
                &mut transfer,
            )
            .unwrap();
    }
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Err(data::Refusal::Order)
    );
    assert_eq!(
        state.execute(Request::Begin { bytes: 1 }, &mut transfer),
        Err(data::Refusal::Order)
    );
    drop(state);
    assert!(!unsafe { private_heap::exhausted() });
    ACTIVE.store(false, Ordering::SeqCst);
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    // The same real allocator must refuse an impossible reservation and remain
    // terminal even when a subsequent small request would fit the empty grant.
    let impossible = Layout::from_size_align(memory::HEAP_BYTES + 1, 32).unwrap();
    assert!(unsafe { PrivateHeap.alloc(impossible) }.is_null());
    assert!(unsafe { private_heap::exhausted() });
    assert!(unsafe { PrivateHeap.alloc(Layout::new::<u8>()) }.is_null());
    println!(
        "PASS exact bitmap grant={} Init peak={} repeated large Graph/ProjectGraph peak={} rounded-live-headroom={}",
        memory::HEAP_BYTES,
        init_peak,
        PEAK.load(Ordering::Relaxed),
        memory::HEAP_BYTES - PEAK.load(Ordering::Relaxed)
    );
}
