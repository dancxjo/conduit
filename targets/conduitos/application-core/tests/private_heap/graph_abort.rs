//! Exact private-grant cleanup for aborts between cooperative entries.
use super::*;
use conduitos_application_core::{
    data::Refusal,
    wire::{GraphPhase, GraphProgress},
};
pub(super) fn run(applications: &[Option<Initial>; 4], transfer: &mut [u8]) {
    for abort_phase in [GraphPhase::Serialize, GraphPhase::Validate] {
        assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        STAGE.store(
            if abort_phase == GraphPhase::Serialize {
                100
            } else {
                101
            },
            Ordering::Relaxed,
        );
        let mut state = State::default();
        initialize::initialize(&mut state, applications, transfer);
        let input = on_caller(|| data::encode_command(&Command::Graph { slot: 0 }).unwrap());
        assert_eq!(
            state.execute(
                Request::Begin {
                    bytes: input.len() as u32
                },
                transfer
            ),
            Ok(Response::Ready)
        );
        transfer[..input.len()].copy_from_slice(&input);
        assert_eq!(
            state.execute(
                Request::Write {
                    offset: 0,
                    bytes: input.len() as u32
                },
                transfer
            ),
            Ok(Response::Ready)
        );
        assert_eq!(
            state.execute(Request::Commit, transfer),
            Ok(Response::Ready)
        );
        if abort_phase == GraphPhase::Validate {
            let mut progress =
                GraphProgress::new(GraphPhase::Serialize, data::GRAPH_SERIALIZE_TURNS).unwrap();
            loop {
                let response =
                    graph_entry_without_heap(&mut state, progress.request().unwrap(), transfer);
                if progress.accept(response).unwrap() {
                    break;
                }
            }
        }
        let request = match abort_phase {
            GraphPhase::Serialize => Request::GraphSerialize { cursor: 0 },
            GraphPhase::Validate => Request::GraphValidate { cursor: 0 },
        };
        assert_eq!(
            graph_entry_without_heap(&mut state, request, transfer),
            Response::Progress {
                phase: abort_phase,
                cursor: 1
            }
        );
        // An interleaved command aborts the retained operation terminally.
        // This is private State cleanup, not a new UI cancellation action or
        // proof of hostile-guest confinement.
        let allocations_before_abort = ALLOCATIONS.load(Ordering::Relaxed);
        assert_eq!(
            state.execute(Request::Begin { bytes: 1 }, transfer),
            Err(Refusal::Order)
        );
        assert_eq!(
            state.execute(
                Request::Read {
                    offset: 0,
                    maximum: 1
                },
                transfer
            ),
            Err(Refusal::Order)
        );
        assert_eq!(
            ALLOCATIONS.load(Ordering::Relaxed),
            allocations_before_abort
        );
        drop(state);
        drop(input);
        assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        let entire = Layout::from_size_align(memory::HEAP_BYTES, 32).unwrap();
        let pointer = unsafe { PrivateHeap.alloc(entire) };
        assert_eq!(pointer as usize, memory::HEAP_START);
        unsafe { PrivateHeap.dealloc(pointer, entire) };
    }
    report("graph serialize/validate abort cleanup cases=", 2);
}
