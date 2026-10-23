//! Test-only CPU protocol service; never selected in a product build.
//! Real application State/codec runs before an explicitly substituted reply.
use conduitos_application_core::{
    data::{self, Command, Refusal, Reply},
    memory::TRANSFER_BYTES,
    state::State,
    wire::{Request, Response},
};
use std::{boxed::Box, cell::RefCell, vec, vec::Vec};

type Substitute = Box<dyn FnOnce(Reply) -> Reply>;
#[derive(Default)]
struct Probe {
    next: Option<Substitute>,
    consumes: usize,
    quarantines: usize,
}
std::thread_local! { static PROBE: RefCell<Probe> = RefCell::default(); }
pub(super) fn substitute(next: impl FnOnce(Reply) -> Reply + 'static) {
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        assert!(probe.next.is_none());
        probe.next = Some(Box::new(next));
    });
}
pub(super) fn reset() {
    PROBE.with(|probe| *probe.borrow_mut() = Probe::default());
}
pub(super) fn counts() -> (usize, usize) {
    PROBE.with(|probe| {
        let probe = probe.borrow();
        (probe.consumes, probe.quarantines)
    })
}
pub(super) struct Service {
    state: State,
    live: bool,
}
impl Service {
    pub fn install() -> Result<Self, Refusal> {
        Ok(Self {
            state: State::default(),
            live: true,
        })
    }
    pub fn quarantine(&mut self) {
        if self.live {
            self.live = false;
            PROBE.with(|probe| probe.borrow_mut().quarantines += 1);
        }
    }
    pub fn command(&mut self, command: &Command) -> Result<Reply, Refusal> {
        if !self.live {
            return Err(Refusal::Order);
        }
        let reply = exchange(&mut self.state, command)?;
        if matches!(command, Command::PatchbayConsume { .. }) {
            let substitute = PROBE.with(|probe| {
                let mut probe = probe.borrow_mut();
                probe.consumes += 1;
                probe.next.take()
            });
            let returned = match substitute {
                Some(substitute) => substitute(reply),
                None => reply,
            };
            Ok(returned)
        } else {
            Ok(reply)
        }
    }
}
fn exchange(state: &mut State, command: &Command) -> Result<Reply, Refusal> {
    let input = data::encode_command(command)?;
    let mut transfer = vec![0; TRANSFER_BYTES];
    state.execute(
        Request::Begin {
            bytes: input.len() as u32,
        },
        &mut transfer,
    )?;
    for (index, chunk) in input.chunks(TRANSFER_BYTES).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        state.execute(
            Request::Write {
                offset: (index * TRANSFER_BYTES) as u32,
                bytes: chunk.len() as u32,
            },
            &mut transfer,
        )?;
    }
    if matches!(
        command,
        Command::PrepareTarget { .. } | Command::PrepareGraphProjection { .. }
    ) {
        assert_eq!(
            state.execute(Request::Preflight, &mut transfer)?,
            Response::Ready
        );
    }
    let mut response = state.execute(Request::Commit, &mut transfer)?;
    if matches!(command, Command::Graph { .. }) {
        assert_eq!(response, Response::Ready);
        for phase in [
            conduitos_application_core::wire::GraphPhase::Serialize,
            conduitos_application_core::wire::GraphPhase::Validate,
        ] {
            let limit = match phase {
                conduitos_application_core::wire::GraphPhase::Serialize => {
                    data::GRAPH_SERIALIZE_TURNS
                }
                conduitos_application_core::wire::GraphPhase::Validate => {
                    data::GRAPH_VALIDATE_TURNS
                }
            };
            let mut progress = conduitos_application_core::wire::GraphProgress::new(phase, limit)?;
            loop {
                let response = state.execute(progress.request()?, &mut transfer)?;
                if progress.accept(response)? {
                    break;
                }
            }
        }
        for offset in (0..data::MAX_DECODED_JSON_BYTES).step_by(data::GRAPH_PACK_BYTES) {
            response = state.execute(
                Request::GraphPack {
                    offset: offset as u32,
                },
                &mut transfer,
            )?;
            if matches!(response, Response::Blob { .. }) {
                break;
            }
        }
    }
    let Response::Blob { bytes } = response else {
        return Err(Refusal::Malformed);
    };
    let mut output = Vec::new();
    while output.len() < bytes as usize {
        let Response::Chunk {
            offset,
            bytes: count,
            total,
        } = state.execute(
            Request::Read {
                offset: output.len() as u32,
                maximum: TRANSFER_BYTES as u32,
            },
            &mut transfer,
        )?
        else {
            return Err(Refusal::Malformed);
        };
        assert_eq!(offset as usize, output.len());
        assert_eq!(total, bytes);
        output.extend_from_slice(&transfer[..count as usize]);
    }
    data::decode_reply(&output)
}
