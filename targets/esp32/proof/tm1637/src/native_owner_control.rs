//! Provisioned owner authentication below installation/lifecycle admission.
//! Authentication creates no Body, Plan, Play, native possession or effect.
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

pub const PROTOCOL: u16 = 2;
pub const MAXIMUM_REQUEST_BYTES: usize = 2048;
const DOMAIN: &[u8] = b"conduit.environment/root-owner-control@2";
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Operation {
    Inspect = 0,
    Reserve = 1,
    Prepare = 2,
    Start = 3,
    Release = 4,
    Cancel = 5,
    Retire = 6,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request<'a> {
    pub protocol: u16,
    pub operation: Operation,
    pub sequence: u64,
    pub host: &'a str,
    pub boot: &'a str,
    pub offer_generation: u64,
    pub body: &'a str,
    pub wake: &'a str,
    pub body_plan: &'a str,
    pub body_play: &'a str,
    pub plan: &'a str,
    pub fragment: &'a str,
    pub fragment_play: &'a str,
    pub assignment_seal: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Disabled,
    InvalidKey,
    Malformed,
    WrongHost,
    StaleBoot,
    WrongGeneration,
    Replay,
    Signature,
}
impl Request<'_> {
    /// Domain-separated, length-delimited semantic transcript. JSON spelling
    /// confers no authority and cannot substitute a differently scoped command.
    pub fn transcript(&self) -> Result<[u8; 32], Refusal> {
        let ids = [
            self.host,
            self.boot,
            self.body,
            self.wake,
            self.body_plan,
            self.body_play,
            self.plan,
            self.fragment,
            self.fragment_play,
        ];
        if self.protocol != PROTOCOL
            || self.sequence == 0
            || self.assignment_seal == [0; 32]
            || ids.iter().any(|id| {
                id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_graphic())
            })
        {
            return Err(Refusal::Malformed);
        }
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update(self.protocol.to_le_bytes());
        hash.update([self.operation as u8]);
        hash.update(self.sequence.to_le_bytes());
        hash.update(self.offer_generation.to_le_bytes());
        for id in ids {
            hash.update((id.len() as u16).to_le_bytes());
            hash.update(id.as_bytes());
        }
        hash.update(self.assignment_seal);
        Ok(hash.finalize().into())
    }
}
/// Non-deserializable, non-Clone result. A Root action must consume it and still
/// validate current assignment/native/lifecycle state; this is only sender proof.
pub struct AuthenticatedRequest<'a> {
    request: Request<'a>,
}
impl<'a> AuthenticatedRequest<'a> {
    pub fn operation(&self) -> Operation {
        self.request.operation
    }
    pub fn into_request(self) -> Request<'a> {
        self.request
    }
}
pub struct OwnerControl {
    key: Option<VerifyingKey>,
    last_sequence: u64,
}
impl OwnerControl {
    /// The trusted installation supplies a public key, never an incoming frame.
    /// No provisioned key leaves control disabled; malformed/weak keys refuse.
    pub fn new(key: Option<[u8; 32]>) -> Result<Self, Refusal> {
        let key = match key {
            None => None,
            Some(bytes) => {
                let key = VerifyingKey::from_bytes(&bytes).map_err(|_| Refusal::InvalidKey)?;
                if key.is_weak() {
                    return Err(Refusal::InvalidKey);
                }
                Some(key)
            }
        };
        Ok(Self {
            key,
            last_sequence: 0,
        })
    }
    pub fn configured(&self) -> bool {
        self.key.is_some()
    }
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
    pub fn authenticate<'a>(
        &mut self,
        request: Request<'a>,
        signature: &[u8; 64],
        host: &str,
        boot: &str,
        generation: u64,
    ) -> Result<AuthenticatedRequest<'a>, Refusal> {
        let key = self.key.as_ref().ok_or(Refusal::Disabled)?;
        let transcript = request.transcript()?;
        if request.host != host {
            return Err(Refusal::WrongHost);
        }
        if request.boot != boot {
            return Err(Refusal::StaleBoot);
        }
        if request.offer_generation != generation {
            return Err(Refusal::WrongGeneration);
        }
        if request.sequence <= self.last_sequence {
            return Err(Refusal::Replay);
        }
        key.verify_strict(&transcript, &Signature::from_bytes(signature))
            .map_err(|_| Refusal::Signature)?;
        self.last_sequence = request.sequence;
        Ok(AuthenticatedRequest { request })
    }
}
