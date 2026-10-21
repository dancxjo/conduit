//! Finite numeric protocol. Envelopes are local service fences, not Host authority.
mod codec;
pub use codec::{decode_request, decode_response, encode_request, encode_response};
pub const EXCHANGE_BYTES: usize = 4096;
pub const TRANSFER_BYTES: u32 = 16 * 1024;
pub const MAX_BLOB_BYTES: u32 = 128 * 1024;
pub const HEADER_BYTES: usize = 32;
pub const VERSION: u16 = 4;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub instance: u64,
    pub request: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InputKind {
    Initialize = 1,
    FaceBasis = 2,
    FaceInteraction = 3,
    NativeFace = 4,
    NativeScene = 5,
    HomeKey = 6,
    HomeView = 7,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OutputKind {
    Snapshot = 1,
    Face = 2,
    Selection = 3,
    Native = 4,
    NativeFace = 5,
    NativeScene = 6,
    Home = 7,
    HomeView = 8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Event {
    Input = 1,
    Change = 2,
    Activate = 3,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request<'a> {
    Begin {
        kind: InputKind,
        total: u32,
    },
    Write {
        offset: u32,
        window_offset: u32,
        bytes: u32,
    },
    Commit,
    AllowIdle,
    HomeInit,
    HomeReset {
        revision: u32,
    },
    HomeRefusal {
        revision: u32,
        code: u8,
    },
    NativeKey {
        revision: u32,
        event: [u8; 3],
    },
    Apply {
        revision: u32,
        event: Event,
        action: &'a str,
        value: &'a str,
    },
    Snapshot {
        revision: u32,
    },
    Selection {
        revision: u32,
    },
    Read {
        offset: u32,
        maximum: u32,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    Ready {
        revision: u32,
    },
    Blob {
        kind: OutputKind,
        revision: u32,
        bytes: u32,
    },
    Chunk {
        offset: u32,
        bytes: u32,
        total: u32,
        complete: bool,
    },
    Refused(Refusal),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Refusal {
    Bounds = 1,
    Malformed = 2,
    Order = 3,
    StaleInstance = 4,
    StaleRequest = 5,
    Capacity = 6,
    Projection = 7,
    Interaction = 8,
    NamingCatalog = 16,
    NamingUuid = 17,
    NamingSystem = 18,
    NamingBounds = 19,
    InvalidName = 20,
    InvalidInventory = 21,
    EmptySelection = 22,
    UnavailablePlot = 23,
    StaleFace = 24,
    RevisionExhausted = 25,
    UnknownChoice = 26,
    UnknownAction = 27,
    InvalidActionValue = 28,
    InvalidSearch = 29,
}
impl From<conduit_birth_plot::BirthDraftRefusal> for Refusal {
    fn from(error: conduit_birth_plot::BirthDraftRefusal) -> Self {
        use conduit_birth_plot::{BirthDraftRefusal as E, names::NamingRefusal as N};
        match error {
            E::Naming(N::Catalog) => Self::NamingCatalog,
            E::Naming(N::Uuid) => Self::NamingUuid,
            E::Naming(N::UnknownSystem) => Self::NamingSystem,
            E::Naming(N::Bounds) => Self::NamingBounds,
            E::InvalidName => Self::InvalidName,
            E::InvalidInventory => Self::InvalidInventory,
            E::EmptySelection => Self::EmptySelection,
            E::UnavailablePlot => Self::UnavailablePlot,
            E::StaleFace => Self::StaleFace,
            E::RevisionExhausted => Self::RevisionExhausted,
            E::UnknownChoice => Self::UnknownChoice,
            E::UnknownAction => Self::UnknownAction,
            E::InvalidActionValue => Self::InvalidActionValue,
            E::InvalidSearch => Self::InvalidSearch,
        }
    }
}
